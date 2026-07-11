const ZOT_BRIDGE = {
  protocolVersion: 1,
  pluginVersion: "0.6.0",
  endpointPrefix: "/zot-bridge/v1",
  tokenHashPref: "extensions.zotero.zot-bridge.tokenHash",
  pairingLifetimeMs: 5 * 60 * 1000,
  replayLifetimeMs: 10 * 60 * 1000,
  pairing: null,
  replay: new Map(),
  menuIds: [],

  async startup() {
    await Zotero.initializationPromise;
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
  },

  unregisterEndpoints() {
    for (const path of ["health", "pair", "status", "auth/revoke"]) {
      delete Zotero.Server.Endpoints[`${this.endpointPrefix}/${path}`];
    }
  },

  async handleHealth(request) {
    try {
      const body = this.parseRequest(request, ["protocol_version", "request_id", "sent_at", "client"], 4096);
      this.validateBase(body, false);
      return this.success(body.request_id, {
        plugin_version: this.pluginVersion,
        zotero_version: Zotero.version,
        protocol_version: this.protocolVersion,
        capabilities: ["health", "pair", "status", "auth-revoke"],
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
      return this.success(body.request_id, {
        token,
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
        capabilities: [
          { name: "status", available: true },
          { name: "auth-revoke", available: true },
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
      return response;
    } catch (error) {
      return this.failure(error);
    }
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

  async prepareProtected(request, maxBytes) {
    const body = this.parseRequest(request, ["protocol_version", "request_id", "sent_at", "client"], maxBytes);
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

function uninstall() {}

function onMainWindowLoad({ window }) {
  ZOT_BRIDGE.addMenu(window);
}

function onMainWindowUnload({ window }) {
  ZOT_BRIDGE.removeMenu(window);
}
