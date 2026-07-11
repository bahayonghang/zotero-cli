const ZOT_BRIDGE = {
  protocolVersion: 1,
  pluginVersion: "0.6.0",
  endpointPrefix: "/zot-bridge/v1",
  tokenHashPref: "extensions.zotero.zot-bridge.tokenHash",
  instanceIdPref: "extensions.zotero.zot-bridge.instanceId",
  pairingLifetimeMs: 5 * 60 * 1000,
  replayLifetimeMs: 10 * 60 * 1000,
  planLifetimeMs: 2 * 60 * 1000,
  pairing: null,
  replay: new Map(),
  plans: new Map(),
  operations: new Map(),
  menuIds: [],

  async startup() {
    await Zotero.initializationPromise;
    this.ensureInstanceId();
    this.registerEndpoints();
    for (const window of Zotero.getMainWindows()) {
      this.addMenu(window);
    }
    if (!this.getTokenHash()) {
      setTimeout(() => this.showPairingCode(Zotero.getMainWindow()), 500);
    }
  },

  shutdown() {
    this.unregisterEndpoints();
    for (const window of Zotero.getMainWindows()) {
      this.removeMenu(window);
    }
    this.pairing = null;
    this.replay.clear();
    this.plans.clear();
    this.operations.clear();
  },

  registerEndpoints() {
    const bridge = this;
    Zotero.Server.Endpoints[`${this.endpointPrefix}/health`] = class {
      supportedMethods = ["POST"];
      supportedDataTypes = ["application/json"];
      permitBookmarklet = false;
      async init(request) {
        return bridge.handleHealth(request);
      }
    };
    Zotero.Server.Endpoints[`${this.endpointPrefix}/pair`] = class {
      supportedMethods = ["POST"];
      supportedDataTypes = ["application/json"];
      permitBookmarklet = false;
      async init(request) {
        return bridge.handlePair(request);
      }
    };
    Zotero.Server.Endpoints[`${this.endpointPrefix}/status`] = class {
      supportedMethods = ["POST"];
      supportedDataTypes = ["application/json"];
      permitBookmarklet = false;
      async init(request) {
        return bridge.handleStatus(request);
      }
    };
    Zotero.Server.Endpoints[`${this.endpointPrefix}/auth/revoke`] = class {
      supportedMethods = ["POST"];
      supportedDataTypes = ["application/json"];
      permitBookmarklet = false;
      async init(request) {
        return bridge.handleRevoke(request);
      }
    };
    Zotero.Server.Endpoints[`${this.endpointPrefix}/merge/preview`] = class {
      supportedMethods = ["POST"];
      supportedDataTypes = ["application/json"];
      permitBookmarklet = false;
      async init(request) {
        return bridge.handleMergePreview(request);
      }
    };
    Zotero.Server.Endpoints[`${this.endpointPrefix}/merge/apply`] = class {
      supportedMethods = ["POST"];
      supportedDataTypes = ["application/json"];
      permitBookmarklet = false;
      async init(request) {
        return bridge.handleMergeApply(request);
      }
    };
  },

  unregisterEndpoints() {
    for (const path of ["health", "pair", "status", "auth/revoke", "merge/preview", "merge/apply"]) {
      delete Zotero.Server.Endpoints[`${this.endpointPrefix}/${path}`];
    }
  },

  async handleHealth(request) {
    try {
      const body = this.parseRequest(request, ["protocol_version", "request_id", "sent_at", "client"], 4096);
      this.validateBase(body, false);
      return this.success(body.request_id, {
        instance_id: this.ensureInstanceId(),
        plugin_version: this.pluginVersion,
        zotero_version: Zotero.version,
        protocol_version: this.protocolVersion,
        capabilities: ["health", "pair", "status", "auth-revoke", "merge-preview", "merge-apply"],
      });
    } catch (error) {
      return this.failure(error);
    }
  },

  async handlePair(request) {
    try {
      const body = this.parseRequest(
        request,
        ["protocol_version", "request_id", "sent_at", "client", "code", "client_instance_id"],
        4096,
      );
      this.validateBase(body, false);
      this.requireString(body.code, "code", 8, 16);
      this.requireString(body.client_instance_id, "client_instance_id", 1, 128);
      const pairing = this.pairing;
      if (!pairing || pairing.used || Date.now() > pairing.expiresAt) {
        this.pairing = null;
        throw this.error(401, "bridge-pair-expired", "Pairing code expired", "Show a new pairing code in Zotero");
      }
      const candidateHash = await this.sha256Hex(body.code);
      if (!this.constantTimeEqual(candidateHash, pairing.codeHash)) {
        throw this.error(401, "bridge-pair-code", "Invalid pairing code", "Check the code shown in Zotero");
      }
      pairing.used = true;
      const token = this.randomBase64Url(32);
      this.setTokenHash(await this.sha256Hex(token));
      this.pairing = null;
      this.replay.clear();
      this.plans.clear();
      this.operations.clear();
      return this.success(body.request_id, {
        token,
        instance_id: this.ensureInstanceId(),
        plugin_version: this.pluginVersion,
        protocol_version: this.protocolVersion,
      });
    } catch (error) {
      return this.failure(error);
    }
  },

  async handleStatus(request) {
    try {
      const prepared = await this.prepareProtected(request, 16384);
      if (prepared.cached) {
        return prepared.cached;
      }
      const response = this.success(prepared.body.request_id, {
        paired: true,
        instance_id: this.ensureInstanceId(),
        capabilities: [
          { name: "status", available: true },
          { name: "auth-revoke", available: true },
          { name: "merge-preview", available: true },
          { name: "merge-apply", available: true },
        ],
        libraries: this.libraryStatus(),
      });
      this.cacheResponse(prepared, response);
      return response;
    } catch (error) {
      return this.failure(error);
    }
  },

  async handleRevoke(request) {
    try {
      const prepared = await this.prepareProtected(request, 16384);
      if (prepared.cached) {
        return prepared.cached;
      }
      const response = this.success(prepared.body.request_id, { revoked: true });
      this.cacheResponse(prepared, response);
      this.clearTokenHash();
      this.pairing = null;
      this.plans.clear();
      this.operations.clear();
      return response;
    } catch (error) {
      return this.failure(error);
    }
  },

  async handleMergePreview(request) {
    try {
      const prepared = await this.prepareProtected(
        request,
        64 * 1024,
        ["scope", "keeper_key", "source_keys"],
      );
      if (prepared.cached) {
        return prepared.cached;
      }
      const candidates = await this.resolveMergeCandidates(
        prepared.body.scope,
        prepared.body.keeper_key,
        prepared.body.source_keys,
      );
      const preview = await this.buildMergePreview(candidates.keeper, candidates.sources);
      const fingerprint = await this.mergeFingerprint(candidates.keeper, candidates.sources);
      const planToken = this.randomBase64Url(32);
      const planId = (await this.sha256Hex(planToken)).slice(0, 12);
      const expiresAt = Date.now() + this.planLifetimeMs;
      this.prunePlans();
      this.plans.set(planToken, {
        authHash: prepared.authHash,
        scope: JSON.parse(JSON.stringify(prepared.body.scope)),
        keeperKey: candidates.keeper.key,
        sourceKeys: candidates.sources.map(item => item.key),
        fingerprint,
        preview,
        expiresAt,
      });
      const response = this.success(prepared.body.request_id, {
        ...preview,
        plan_id: planId,
        expires_at: new Date(expiresAt).toISOString(),
        plan_token: planToken,
      });
      this.cacheResponse(prepared, response);
      return response;
    } catch (error) {
      return this.failure(error);
    }
  },

  async handleMergeApply(request) {
    try {
      const prepared = await this.prepareProtected(
        request,
        64 * 1024,
        ["plan_token", "operation_id"],
      );
      if (prepared.cached) {
        return prepared.cached;
      }
      this.requireString(prepared.body.plan_token, "plan_token", 32, 128);
      this.requireString(prepared.body.operation_id, "operation_id", 1, 128);
      this.prunePlans();
      this.pruneOperations();
      const operationHash = await this.sha256Hex(prepared.body.plan_token);
      const existing = this.operations.get(prepared.body.operation_id);
      if (existing) {
        if (this.constantTimeEqual(existing.authHash, prepared.authHash)
          && this.constantTimeEqual(existing.operationHash, operationHash)) {
          const response = this.success(prepared.body.request_id, {
            ...existing.result,
            already_applied: true,
          });
          this.cacheResponse(prepared, response);
          return response;
        }
        throw this.error(409, "bridge-replay", "operation_id was reused with different merge data", null);
      }
      const plan = this.plans.get(prepared.body.plan_token);
      if (!plan || plan.expiresAt <= Date.now()) {
        this.plans.delete(prepared.body.plan_token);
        throw this.error(409, "bridge-plan-expired", "Merge preview plan expired", "Run the dry-run preview again");
      }
      if (!this.constantTimeEqual(plan.authHash, prepared.authHash)) {
        throw this.error(401, "bridge-auth", "Merge plan belongs to another authorization", "Run the preview again");
      }
      const candidates = await this.resolveMergeCandidates(plan.scope, plan.keeperKey, plan.sourceKeys);
      const fingerprint = await this.mergeFingerprint(candidates.keeper, candidates.sources);
      if (!this.constantTimeEqual(plan.fingerprint, fingerprint)) {
        throw this.error(409, "bridge-item-changed", "Merge candidates changed after preview", "Review a new dry-run preview before applying");
      }
      const originalFields = plan.preview.metadata_fields_to_fill.map(fill => ({
        field: fill.field,
        value: candidates.keeper.getField(fill.field),
      }));
      for (const fill of plan.preview.metadata_fields_to_fill) {
        candidates.keeper.setField(fill.field, fill.value);
      }
      try {
        const { mergeItems } = ChromeUtils.importESModule("chrome://zotero/content/mergeItems.mjs");
        await mergeItems(candidates.keeper, candidates.sources);
      } catch (_error) {
        for (const original of originalFields) {
          candidates.keeper.setField(original.field, original.value);
        }
        try {
          await Zotero.Items.reload(
            [candidates.keeper.id, ...candidates.sources.map(item => item.id)],
            null,
            true,
          );
        } catch (_reloadError) {
          // The database transaction already rolled back; this only refreshes object cache state.
        }
        throw this.error(500, "bridge-transaction", "Zotero native merge transaction failed", "Review the Zotero error console; no database changes were committed");
      }
      const result = {
        already_applied: false,
        keeper_key: plan.preview.keeper_key,
        source_keys_trashed: plan.preview.source_keys,
        metadata_fields_filled: plan.preview.metadata_fields_to_fill.map(entry => entry.field),
        skipped_incompatible_fields: plan.preview.skipped_incompatible_fields,
        tags_added: plan.preview.tags_to_add,
        collections_added: plan.preview.collections_to_add,
        relations_to_add: plan.preview.relations_to_add,
        children_reparented: plan.preview.children_to_reparent,
        skipped_duplicate_attachments: plan.preview.skipped_duplicate_attachments,
      };
      const response = this.success(prepared.body.request_id, result);
      this.operations.set(prepared.body.operation_id, {
        authHash: prepared.authHash,
        operationHash,
        result,
        expiresAt: Date.now() + this.replayLifetimeMs,
      });
      this.plans.delete(prepared.body.plan_token);
      this.cacheResponse(prepared, response);
      return response;
    } catch (error) {
      return this.failure(error);
    }
  },

  validateScope(scope) {
    if (!scope || typeof scope !== "object" || Array.isArray(scope)) {
      throw this.error(400, "bridge-invalid-request", "scope must be an object", null);
    }
    const allowed = scope.type === "user" ? ["type"] : ["type", "group_id"];
    if (Object.keys(scope).some(key => !allowed.includes(key))) {
      throw this.error(400, "bridge-unknown-field", "scope contains unknown fields", null);
    }
    if (scope.type === "user") {
      return Zotero.Libraries.userLibraryID;
    }
    if (scope.type === "group" && Number.isSafeInteger(scope.group_id) && scope.group_id > 0) {
      const group = Zotero.Groups.getByGroupID(scope.group_id);
      if (group) {
        return group.libraryID;
      }
      throw this.error(404, "bridge-library-not-found", "Zotero group library was not found", "Open or sync the group library in Zotero");
    }
    throw this.error(400, "bridge-invalid-request", "scope type must be user or group", null);
  },

  async resolveMergeCandidates(scope, keeperKey, sourceKeys) {
    const libraryID = this.validateScope(scope);
    this.requireString(keeperKey, "keeper_key", 1, 64);
    if (!Array.isArray(sourceKeys) || sourceKeys.length === 0 || sourceKeys.length > 50) {
      throw this.error(400, "bridge-invalid-request", "source_keys must contain 1 to 50 item keys", null);
    }
    for (const key of sourceKeys) {
      this.requireString(key, "source key", 1, 64);
    }
    const unique = new Set([keeperKey, ...sourceKeys]);
    if (unique.size !== sourceKeys.length + 1) {
      throw this.error(400, "bridge-invalid-request", "Merge item keys must be unique", null);
    }
    const library = Zotero.Libraries.get(libraryID);
    if (!library) {
      throw this.error(404, "bridge-library-not-found", "Zotero library was not found", "Open or sync the library in Zotero");
    }
    if (!library.editable) {
      throw this.error(403, "bridge-library-readonly", "Zotero library is read-only", "Request write access or choose an editable library");
    }
    const items = [];
    for (const key of [keeperKey, ...sourceKeys]) {
      const item = await Zotero.Items.getByLibraryAndKeyAsync(libraryID, key);
      if (!item) {
        throw this.error(404, "bridge-item-not-found", `Merge item was not found: ${key}`, "Refresh Zotero and preview again");
      }
      if (item.libraryID !== libraryID) {
        throw this.error(400, "bridge-cross-library", "Merge candidates must be in one library", null);
      }
      if (item.deleted) {
        throw this.error(409, "bridge-item-changed", `Merge item is already deleted: ${key}`, "Preview the current library state again");
      }
      if (!item.isRegularItem() || item.parentID || item.parentItemID) {
        throw this.error(400, "bridge-invalid-child", `Merge item is not a top-level bibliographic item: ${key}`, null);
      }
      items.push(item);
    }
    return { keeper: items[0], sources: items.slice(1) };
  },

  async buildMergePreview(keeper, sources) {
    const planned = new Map();
    for (const fieldID of Zotero.ItemFields.getItemTypeFields(keeper.itemTypeID)) {
      const field = Zotero.ItemFields.getName(fieldID);
      planned.set(field, keeper.getField(fieldID));
    }
    const fills = [];
    const skipped = [];
    for (const source of sources) {
      for (const sourceFieldID of Zotero.ItemFields.getItemTypeFields(source.itemTypeID)) {
        const sourceValue = source.getField(sourceFieldID);
        if (this.isEmptyValue(sourceValue)) {
          continue;
        }
        const baseFieldID = Zotero.ItemFields.getBaseIDFromTypeAndField(
          source.itemTypeID,
          sourceFieldID,
        ) || sourceFieldID;
        const keeperFieldID = Zotero.ItemFields.getFieldIDFromTypeAndBase(
          keeper.itemTypeID,
          baseFieldID,
        );
        if (!keeperFieldID) {
          skipped.push({
            field: Zotero.ItemFields.getName(sourceFieldID),
            source_key: source.key,
          });
          continue;
        }
        const keeperField = Zotero.ItemFields.getName(keeperFieldID);
        if (this.isEmptyValue(planned.get(keeperField))) {
          const value = String(sourceValue);
          planned.set(keeperField, value);
          fills.push({ field: keeperField, source_key: source.key, value });
        }
      }
    }
    const keeperTags = new Set(keeper.getTags().map(entry => entry.tag));
    const keeperCollections = new Set(keeper.getCollections().map(String));
    const tagsToAdd = new Set();
    const collectionsToAdd = new Set();
    let childrenToReparent = 0;
    for (const source of sources) {
      for (const tag of source.getTags().map(entry => entry.tag)) {
        if (!keeperTags.has(tag)) tagsToAdd.add(tag);
      }
      for (const collection of source.getCollections().map(String)) {
        if (!keeperCollections.has(collection)) collectionsToAdd.add(collection);
      }
      childrenToReparent += source.getAttachments(true).length + source.getNotes(true).length;
    }
    return {
      keeper_key: keeper.key,
      source_keys: sources.map(item => item.key),
      metadata_fields_to_fill: fills,
      skipped_incompatible_fields: skipped,
      tags_to_add: [...tagsToAdd].sort(),
      collections_to_add: [...collectionsToAdd].sort(),
      relations_to_add: sources.map(item => Zotero.URI.getItemURI(item)).sort(),
      children_to_reparent: childrenToReparent,
      skipped_duplicate_attachments: 0,
      confirm_required: true,
    };
  },

  async mergeFingerprint(keeper, sources) {
    const snapshots = [];
    for (const item of [keeper, ...sources]) {
      const fields = {};
      const fieldIDs = [...Zotero.ItemFields.getItemTypeFields(item.itemTypeID)]
        .sort((a, b) => Zotero.ItemFields.getName(a).localeCompare(Zotero.ItemFields.getName(b)));
      for (const fieldID of fieldIDs) {
        fields[Zotero.ItemFields.getName(fieldID)] = item.getField(fieldID);
      }
      const childIDs = [...item.getAttachments(true), ...item.getNotes(true)];
      const children = childIDs.length ? await Zotero.Items.getAsync(childIDs) : [];
      snapshots.push({
        key: item.key,
        version: item.version,
        itemType: item.itemType,
        libraryID: item.libraryID,
        deleted: Boolean(item.deleted),
        fields,
        creators: item.getCreators(),
        tags: item.getTags().map(entry => [entry.tag, entry.type]).sort(),
        collections: item.getCollections().map(String).sort(),
        relations: item.getRelations(),
        children: children.map(child => ({
          key: child.key,
          version: child.version,
          itemType: child.itemType,
          linkMode: child.attachmentLinkMode ?? null,
          contentType: child.attachmentContentType ?? null,
        })).sort((a, b) => a.key.localeCompare(b.key)),
      });
    }
    return this.sha256Hex(this.canonicalStringify(snapshots));
  },

  canonicalStringify(value) {
    if (Array.isArray(value)) {
      return `[${value.map(entry => this.canonicalStringify(entry)).join(",")}]`;
    }
    if (value && typeof value === "object") {
      return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${this.canonicalStringify(value[key])}`).join(",")}}`;
    }
    return JSON.stringify(value);
  },

  isEmptyValue(value) {
    return value === null || value === undefined || (typeof value === "string" && value.trim() === "");
  },

  parseRequest(request, allowedFields, maxBytes) {
    this.validateTransport(request);
    const raw = typeof request.data === "string" ? request.data : JSON.stringify(request.data || {});
    if (raw.length > maxBytes) {
      throw this.error(413, "bridge-request-too-large", "Request body is too large", null);
    }
    let body;
    try {
      body = typeof request.data === "string" ? JSON.parse(request.data) : request.data;
    } catch (_error) {
      throw this.error(400, "bridge-invalid-json", "Request body is not valid JSON", null);
    }
    if (!body || typeof body !== "object" || Array.isArray(body)) {
      throw this.error(400, "bridge-invalid-request", "Request body must be a JSON object", null);
    }
    const allowed = new Set(allowedFields);
    for (const key of Object.keys(body)) {
      if (!allowed.has(key)) {
        throw this.error(400, "bridge-unknown-field", `Unknown request field: ${key}`, null);
      }
    }
    return body;
  },

  validateTransport(request) {
    const origin = this.header(request, "origin");
    if (origin) {
      throw this.error(403, "bridge-origin", "Browser-originated requests are not allowed", null);
    }
    const host = this.header(request, "host");
    const allowedHosts = new Set(["127.0.0.1:23119", "localhost:23119", "[::1]:23119"]);
    if (!host || !allowedHosts.has(host.toLowerCase())) {
      throw this.error(403, "bridge-host", "Request Host is not an allowed loopback address", null);
    }
  },

  validateBase(body, checkClock) {
    if (body.protocol_version !== this.protocolVersion) {
      throw this.error(409, "bridge-protocol", "Unsupported bridge protocol version", "Update zot or reinstall the matching bridge plugin");
    }
    this.requireString(body.request_id, "request_id", 1, 128);
    this.requireString(body.sent_at, "sent_at", 1, 64);
    if (!body.client || typeof body.client !== "object" || Array.isArray(body.client)) {
      throw this.error(400, "bridge-client", "client must be an object", null);
    }
    const clientKeys = Object.keys(body.client);
    if (clientKeys.some((key) => !["name", "version"].includes(key))) {
      throw this.error(400, "bridge-client", "client contains unknown fields", null);
    }
    this.requireString(body.client.name, "client.name", 1, 64);
    this.requireString(body.client.version, "client.version", 1, 64);
    if (checkClock) {
      const sentAt = Date.parse(body.sent_at);
      if (!Number.isFinite(sentAt) || Math.abs(Date.now() - sentAt) > 60 * 1000) {
        throw this.error(409, "bridge-clock", "Request timestamp is outside the allowed window", "Check the system clock and retry");
      }
    }
  },

  async prepareProtected(request, maxBytes, extraFields = []) {
    const body = this.parseRequest(
      request,
      ["protocol_version", "request_id", "sent_at", "client", ...extraFields],
      maxBytes,
    );
    this.validateBase(body, true);
    const token = this.bearerToken(request);
    const authHash = await this.sha256Hex(token);
    const payloadHash = await this.sha256Hex(JSON.stringify(body));
    this.pruneReplay();
    const existing = this.replay.get(body.request_id);
    if (existing) {
      if (this.constantTimeEqual(existing.authHash, authHash) && this.constantTimeEqual(existing.payloadHash, payloadHash)) {
        return { body, cached: existing.response };
      }
      throw this.error(409, "bridge-replay", "request_id was reused with different request data", null);
    }
    const configuredHash = this.getTokenHash();
    if (!configuredHash || !this.constantTimeEqual(configuredHash, authHash)) {
      throw this.error(401, "bridge-auth", "Desktop bridge authorization failed", "Show a pairing code in Zotero and run zot bridge pair");
    }
    return { body, authHash, payloadHash, cached: null };
  },

  cacheResponse(prepared, response) {
    this.replay.set(prepared.body.request_id, {
      authHash: prepared.authHash,
      payloadHash: prepared.payloadHash,
      response,
      expiresAt: Date.now() + this.replayLifetimeMs,
    });
  },

  pruneReplay() {
    const now = Date.now();
    for (const [key, value] of this.replay) {
      if (value.expiresAt <= now) {
        this.replay.delete(key);
      }
    }
  },

  prunePlans() {
    const now = Date.now();
    for (const [key, value] of this.plans) {
      if (value.expiresAt <= now) {
        this.plans.delete(key);
      }
    }
  },

  pruneOperations() {
    const now = Date.now();
    for (const [key, value] of this.operations) {
      if (value.expiresAt <= now) {
        this.operations.delete(key);
      }
    }
  },

  bearerToken(request) {
    const authorization = this.header(request, "authorization");
    if (!authorization || !authorization.startsWith("Bearer ")) {
      throw this.error(401, "bridge-auth", "Missing desktop bridge bearer token", "Run zot bridge pair <code>");
    }
    const token = authorization.slice("Bearer ".length);
    this.requireString(token, "bearer token", 16, 256);
    return token;
  },

  header(request, name) {
    const headers = request.headers || {};
    if (typeof headers.get === "function") {
      return headers.get(name) || headers.get(name.toLowerCase());
    }
    for (const [key, value] of Object.entries(headers)) {
      if (key.toLowerCase() === name.toLowerCase()) {
        return String(value);
      }
    }
    return null;
  },

  requireString(value, name, minLength, maxLength) {
    if (typeof value !== "string" || value.length < minLength || value.length > maxLength) {
      throw this.error(400, "bridge-invalid-request", `${name} has an invalid length`, null);
    }
  },

  libraryStatus() {
    const result = [];
    for (const library of Zotero.Libraries.getAll()) {
      if (library.libraryType === "feed") {
        continue;
      }
      let groupId = null;
      if (library.libraryType === "group") {
        try {
          const group = Zotero.Groups.getByLibraryID(library.libraryID);
          groupId = group ? group.groupID : null;
        } catch (_error) {
          groupId = null;
        }
      }
      result.push({
        library_type: library.libraryType,
        group_id: groupId,
        editable: Boolean(library.editable),
      });
    }
    return result;
  },

  async ensurePairingCode() {
    if (this.pairing && !this.pairing.used && Date.now() <= this.pairing.expiresAt) {
      return this.pairing;
    }
    const code = this.randomPairingCode();
    this.pairing = {
      code,
      codeHash: await this.sha256Hex(code),
      expiresAt: Date.now() + this.pairingLifetimeMs,
      used: false,
    };
    return this.pairing;
  },

  async showPairingCode(window) {
    if (!window) {
      return;
    }
    const pairing = await this.ensurePairingCode();
    Services.prompt.alert(
      window,
      "Zot Bridge Pairing Code",
      `Pairing code: ${pairing.code}\n\nThis code expires in five minutes and can be used once.`,
    );
  },

  resetAuthorization(window) {
    this.clearTokenHash();
    this.pairing = null;
    this.replay.clear();
    this.plans.clear();
    this.operations.clear();
    this.showPairingCode(window);
  },

  addMenu(window) {
    if (!window || !window.document) {
      return;
    }
    const popup = window.document.getElementById("menu_ToolsPopup");
    if (!popup) {
      return;
    }
    const codeId = "zot-bridge-show-code";
    const resetId = "zot-bridge-reset-auth";
    if (!window.document.getElementById(codeId)) {
      const item = window.document.createXULElement("menuitem");
      item.id = codeId;
      item.setAttribute("label", "Zot Bridge: Show Pairing Code");
      item.addEventListener("command", () => this.showPairingCode(window));
      popup.appendChild(item);
    }
    if (!window.document.getElementById(resetId)) {
      const item = window.document.createXULElement("menuitem");
      item.id = resetId;
      item.setAttribute("label", "Zot Bridge: Reset Authorization");
      item.addEventListener("command", () => this.resetAuthorization(window));
      popup.appendChild(item);
    }
  },

  removeMenu(window) {
    for (const id of ["zot-bridge-show-code", "zot-bridge-reset-auth"]) {
      const item = window.document.getElementById(id);
      if (item) {
        item.remove();
      }
    }
  },

  getTokenHash() {
    return Zotero.Prefs.get(this.tokenHashPref, true) || "";
  },

  ensureInstanceId() {
    const existing = Zotero.Prefs.get(this.instanceIdPref, true) || "";
    if (existing) {
      return existing;
    }
    const created = this.randomBase64Url(16);
    Zotero.Prefs.set(this.instanceIdPref, created, true);
    return created;
  },

  setTokenHash(value) {
    Zotero.Prefs.set(this.tokenHashPref, value, true);
  },

  clearTokenHash() {
    Zotero.Prefs.clear(this.tokenHashPref, true);
  },

  randomPairingCode() {
    const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    const bytes = new Uint8Array(8);
    crypto.getRandomValues(bytes);
    return Array.from(bytes, (byte) => alphabet[byte & 31]).join("");
  },

  randomBase64Url(length) {
    const bytes = new Uint8Array(length);
    crypto.getRandomValues(bytes);
    let binary = "";
    for (const byte of bytes) {
      binary += String.fromCharCode(byte);
    }
    return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
  },

  async sha256Hex(value) {
    const bytes = new TextEncoder().encode(value);
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
    return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
  },

  constantTimeEqual(left, right) {
    if (typeof left !== "string" || typeof right !== "string" || left.length !== right.length) {
      return false;
    }
    let difference = 0;
    for (let index = 0; index < left.length; index += 1) {
      difference |= left.charCodeAt(index) ^ right.charCodeAt(index);
    }
    return difference === 0;
  },

  success(requestId, data) {
    return [
      200,
      "application/json",
      JSON.stringify({
        ok: true,
        data,
        meta: {
          request_id: requestId,
          protocol_version: this.protocolVersion,
          plugin_version: this.pluginVersion,
          zotero_version: Zotero.version,
        },
      }),
    ];
  },

  failure(error) {
    const normalized = error && error.bridgeError
      ? error
      : this.error(500, "bridge-internal", "Desktop bridge operation failed", "Check the Zotero error console");
    return [
      normalized.status,
      "application/json",
      JSON.stringify({
        ok: false,
        error: {
          code: normalized.code,
          message: normalized.message,
          hint: normalized.hint,
          retryable: normalized.status >= 500,
        },
        meta: {
          request_id: "unknown",
          protocol_version: this.protocolVersion,
          plugin_version: this.pluginVersion,
          zotero_version: Zotero.version,
        },
      }),
    ];
  },

  error(status, code, message, hint) {
    return { bridgeError: true, status, code, message, hint };
  },
};

function install() {}

async function startup() {
  await ZOT_BRIDGE.startup();
}

function shutdown() {
  ZOT_BRIDGE.shutdown();
}

// Authorization and instance identity intentionally survive uninstall/reinstall
// in the same Zotero profile. Users can explicitly revoke or reset them.
function uninstall() {}

function onMainWindowLoad({ window }) {
  ZOT_BRIDGE.addMenu(window);
}

function onMainWindowUnload({ window }) {
  ZOT_BRIDGE.removeMenu(window);
}
