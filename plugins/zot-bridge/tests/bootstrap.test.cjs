const assert = require("node:assert/strict");
const cryptoModule = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { TextEncoder } = require("node:util");

function mockItem({
  key,
  libraryID = 1,
  itemType = "journalArticle",
  version = 1,
  fields = {},
  tags = [],
  collections = [],
  relations = {},
  childIDs = [],
  regular = true,
  parentID = null,
}) {
  return {
    key,
    libraryID,
    itemType,
    itemTypeID: itemType,
    version,
    deleted: false,
    parentID,
    parentItemID: parentID,
    fields: { ...fields },
    isRegularItem() {
      return regular;
    },
    getField(field) {
      return this.fields[field] ?? "";
    },
    setField(field, value) {
      this.fields[field] = value;
    },
    getTags() {
      return tags.map(tag => ({ tag, type: 0 }));
    },
    getCollections() {
      return [...collections];
    },
    getRelations() {
      return JSON.parse(JSON.stringify(relations));
    },
    getCreators() {
      return [];
    },
    getAttachments() {
      return [...childIDs];
    },
    getNotes() {
      return [];
    },
  };
}

function loadBridge(options = {}) {
  const prefs = options.prefs || new Map();
  const items = options.items || [];
  const children = options.children || [];
  const itemByKey = new Map(items.map(item => [item.key, item]));
  const childByID = new Map(children.map(item => [item.id, item]));
  let mergeCalls = 0;
  const context = vm.createContext({
    Zotero: {
      version: "9.0.6",
      initializationPromise: Promise.resolve(),
      Server: { Endpoints: {} },
      Prefs: {
        get(key) {
          return prefs.get(key) || "";
        },
        set(key, value) {
          prefs.set(key, value);
        },
        clear(key) {
          prefs.delete(key);
        },
      },
      Libraries: {
        userLibraryID: 1,
        get(libraryID) {
          if (options.libraryMissing) return null;
          return { libraryID, editable: options.editable !== false };
        },
        getAll() {
          return [{ libraryType: "user", editable: options.editable !== false, libraryID: 1 }];
        },
      },
      Groups: {
        getByGroupID(groupID) {
          return groupID === 42 ? { groupID, libraryID: 2 } : null;
        },
        getByLibraryID() {
          return null;
        },
      },
      Items: {
        async getByLibraryAndKeyAsync(_libraryID, key) {
          return itemByKey.get(key) || false;
        },
        async getAsync(ids) {
          return ids.map(id => childByID.get(id)).filter(Boolean);
        },
        async reload() {},
      },
      ItemFields: {
        getItemTypeFields(itemType) {
          const fields = new Set();
          for (const item of items) {
            if (item.itemTypeID === itemType) {
              Object.keys(item.fields).forEach(field => fields.add(field));
            }
          }
          return [...fields];
        },
        getName(field) {
          return field;
        },
        getBaseIDFromTypeAndField(_itemType, field) {
          return field;
        },
        getFieldIDFromTypeAndBase(itemType, field) {
          return this.getItemTypeFields(itemType).includes(field) ? field : false;
        },
      },
      URI: {
        getItemURI(item) {
          const scope = item.libraryID === 1 ? "users/1" : "groups/42";
          return `http://zotero.org/${scope}/items/${item.key}`;
        },
      },
      getMainWindows() {
        return [];
      },
      getMainWindow() {
        return null;
      },
    },
    Services: { prompt: { alert() {} } },
    ChromeUtils: {
      importESModule(specifier) {
        assert.equal(specifier, "chrome://zotero/content/mergeItems.mjs");
        return {
          async mergeItems(keeper, sources) {
            mergeCalls += 1;
            if (options.mergeError) {
              throw new Error("injected merge failure");
            }
            for (const source of sources) {
              source.deleted = true;
            }
            if (options.onMerge) {
              await options.onMerge(keeper, sources);
            }
          },
        };
      },
    },
    crypto: cryptoModule.webcrypto,
    TextEncoder,
    Uint8Array,
    btoa(value) {
      return Buffer.from(value, "binary").toString("base64");
    },
    setTimeout() {
      return 0;
    },
  });
  const source = fs.readFileSync(path.join(__dirname, "..", "bootstrap.js"), "utf8");
  vm.runInContext(source, context, { filename: "bootstrap.js" });
  return {
    bridge: vm.runInContext("ZOT_BRIDGE", context),
    endpoints: context.Zotero.Server.Endpoints,
    prefs,
    items: itemByKey,
    mergeCalls() {
      return mergeCalls;
    },
  };
}

function baseRequest(requestId = "request-1") {
  return {
    protocol_version: 1,
    request_id: requestId,
    sent_at: new Date().toISOString(),
    client: { name: "zot", version: "0.6.0" },
  };
}

function request(data, headers = {}) {
  return {
    data,
    headers: {
      host: "127.0.0.1:23119",
      ...headers,
    },
  };
}

function responseJson(response) {
  return JSON.parse(response[2]);
}

async function authorize(bridge) {
  const pairing = await bridge.ensurePairingCode();
  const response = await bridge.handlePair(
    request({
      ...baseRequest(`pair-${Math.random()}`),
      code: pairing.code,
      client_instance_id: "client-1",
    }),
  );
  assert.equal(response[0], 200);
  return responseJson(response).data.token;
}

test("health rejects browser origins and unknown fields", async () => {
  const { bridge } = loadBridge();
  const origin = await bridge.handleHealth(
    request(baseRequest(), { origin: "https://example.com" }),
  );
  assert.equal(origin[0], 403);
  assert.equal(responseJson(origin).error.code, "bridge-origin");

  const unknown = await bridge.handleHealth(
    request({ ...baseRequest(), script: "return Zotero" }),
  );
  assert.equal(unknown[0], 400);
  assert.equal(responseJson(unknown).error.code, "bridge-unknown-field");
});

test("pair is single-use and protected status supports replay", async () => {
  const { bridge } = loadBridge();
  const pairing = await bridge.ensurePairingCode();
  assert.match(pairing.code, /^[A-HJ-NP-Z2-9]{8}$/);

  const pairResponse = await bridge.handlePair(
    request({
      ...baseRequest("pair-1"),
      code: pairing.code,
      client_instance_id: "client-1",
    }),
  );
  assert.equal(pairResponse[0], 200);
  const token = responseJson(pairResponse).data.token;
  assert.ok(token.length >= 40);

  const reused = await bridge.handlePair(
    request({
      ...baseRequest("pair-2"),
      code: pairing.code,
      client_instance_id: "client-1",
    }),
  );
  assert.equal(reused[0], 401);
  assert.equal(responseJson(reused).error.code, "bridge-pair-expired");

  const statusRequest = request(baseRequest("status-1"), {
    authorization: `Bearer ${token}`,
  });
  const first = await bridge.handleStatus(statusRequest);
  const second = await bridge.handleStatus(statusRequest);
  assert.equal(first[0], 200);
  assert.deepEqual(second, first);
  assert.equal(responseJson(first).data.paired, true);
});

test("authorization and instance identity survive plugin reload in the same profile", async () => {
  const prefs = new Map();
  const firstLoad = loadBridge({ prefs });
  const token = await authorize(firstLoad.bridge);
  const firstHealth = responseJson(await firstLoad.bridge.handleHealth(
    request(baseRequest("health-before-reload")),
  )).data;
  assert.match(firstHealth.instance_id, /^[A-Za-z0-9_-]{20,}$/);
  firstLoad.bridge.shutdown();

  const secondLoad = loadBridge({ prefs });
  const secondHealth = responseJson(await secondLoad.bridge.handleHealth(
    request(baseRequest("health-after-reload")),
  )).data;
  const status = await secondLoad.bridge.handleStatus(
    request(baseRequest("status-after-reload"), {
      authorization: `Bearer ${token}`,
    }),
  );

  assert.equal(status[0], 200);
  assert.equal(secondHealth.instance_id, firstHealth.instance_id);
});

test("expired pairing codes are rejected", async () => {
  const { bridge } = loadBridge();
  const pairing = await bridge.ensurePairingCode();
  pairing.expiresAt = Date.now() - 1;

  const response = await bridge.handlePair(
    request({
      ...baseRequest("expired-pair"),
      code: pairing.code,
      client_instance_id: "client-1",
    }),
  );

  assert.equal(response[0], 401);
  assert.equal(responseJson(response).error.code, "bridge-pair-expired");
});

test("revoke invalidates the token while replaying the same request", async () => {
  const { bridge, prefs } = loadBridge();
  const pairing = await bridge.ensurePairingCode();
  const pairResponse = await bridge.handlePair(
    request({
      ...baseRequest("pair-1"),
      code: pairing.code,
      client_instance_id: "client-1",
    }),
  );
  const token = responseJson(pairResponse).data.token;
  const revokeRequest = request(baseRequest("revoke-1"), {
    authorization: `Bearer ${token}`,
  });
  const first = await bridge.handleRevoke(revokeRequest);
  const replay = await bridge.handleRevoke(revokeRequest);
  assert.equal(first[0], 200);
  assert.deepEqual(replay, first);
  assert.equal(prefs.has(bridge.tokenHashPref), false);
  assert.equal(prefs.has(bridge.instanceIdPref), true);

  const newStatus = await bridge.handleStatus(
    request(baseRequest("status-after-revoke"), {
      authorization: `Bearer ${token}`,
    }),
  );
  assert.equal(newStatus[0], 401);
});

test("merge preview fills compatible metadata and apply is idempotent", async () => {
  const keeper = mockItem({
    key: "KEEP0001",
    fields: { title: "Keeper", DOI: "" },
    tags: ["keeper-tag"],
    collections: [1],
  });
  const source = mockItem({
    key: "DUPE0001",
    itemType: "preprint",
    fields: { title: "Source", DOI: "10.1000/test", repository: "arXiv" },
    tags: ["source-tag"],
    collections: [2],
  });
  const { bridge, mergeCalls } = loadBridge({ items: [keeper, source] });
  const token = await authorize(bridge);
  const previewResponse = await bridge.handleMergePreview(
    request(
      {
        ...baseRequest("preview-1"),
        scope: { type: "user" },
        keeper_key: keeper.key,
        source_keys: [source.key],
      },
      { authorization: `Bearer ${token}` },
    ),
  );
  assert.equal(previewResponse[0], 200);
  const preview = responseJson(previewResponse).data;
  assert.deepEqual(
    preview.metadata_fields_to_fill.map(entry => [entry.field, entry.value]),
    [["DOI", "10.1000/test"]],
  );
  assert.deepEqual(preview.skipped_incompatible_fields, [
    { field: "repository", source_key: "DUPE0001" },
  ]);
  assert.deepEqual(preview.tags_to_add, ["source-tag"]);
  assert.deepEqual(preview.collections_to_add, ["2"]);
  assert.equal(preview.confirm_required, true);
  assert.match(preview.plan_token, /^[A-Za-z0-9_-]{40,}$/);

  const applyBody = {
    plan_token: preview.plan_token,
    operation_id: "operation-1",
  };
  const firstApply = await bridge.handleMergeApply(
    request(
      { ...baseRequest("apply-1"), ...applyBody },
      { authorization: `Bearer ${token}` },
    ),
  );
  assert.equal(firstApply[0], 200);
  assert.equal(responseJson(firstApply).data.already_applied, false);
  assert.equal(keeper.fields.DOI, "10.1000/test");
  assert.equal(source.deleted, true);
  assert.equal(mergeCalls(), 1);

  const retry = await bridge.handleMergeApply(
    request(
      { ...baseRequest("apply-2"), ...applyBody },
      { authorization: `Bearer ${token}` },
    ),
  );
  assert.equal(retry[0], 200);
  assert.equal(responseJson(retry).meta.request_id, "apply-2");
  assert.equal(responseJson(retry).data.already_applied, true);
  assert.equal(mergeCalls(), 1);
});

test("merge apply rejects expired plans and item drift", async () => {
  const keeper = mockItem({ key: "KEEP0001", fields: { title: "Keeper", DOI: "" } });
  const source = mockItem({
    key: "DUPE0001",
    itemType: "preprint",
    fields: { title: "Source", DOI: "10.1000/test" },
  });
  const { bridge } = loadBridge({ items: [keeper, source] });
  const token = await authorize(bridge);
  const preview = responseJson(await bridge.handleMergePreview(
    request(
      {
        ...baseRequest("preview-drift"),
        scope: { type: "user" },
        keeper_key: keeper.key,
        source_keys: [source.key],
      },
      { authorization: `Bearer ${token}` },
    ),
  )).data;
  source.version += 1;
  const drift = await bridge.handleMergeApply(
    request(
      {
        ...baseRequest("apply-drift"),
        plan_token: preview.plan_token,
        operation_id: "operation-drift",
      },
      { authorization: `Bearer ${token}` },
    ),
  );
  assert.equal(drift[0], 409);
  assert.equal(responseJson(drift).error.code, "bridge-item-changed");

  source.version -= 1;
  const secondPreview = responseJson(await bridge.handleMergePreview(
    request(
      {
        ...baseRequest("preview-expired"),
        scope: { type: "user" },
        keeper_key: keeper.key,
        source_keys: [source.key],
      },
      { authorization: `Bearer ${token}` },
    ),
  )).data;
  bridge.plans.get(secondPreview.plan_token).expiresAt = Date.now() - 1;
  const expired = await bridge.handleMergeApply(
    request(
      {
        ...baseRequest("apply-expired"),
        plan_token: secondPreview.plan_token,
        operation_id: "operation-expired",
      },
      { authorization: `Bearer ${token}` },
    ),
  );
  assert.equal(expired[0], 409);
  assert.equal(responseJson(expired).error.code, "bridge-plan-expired");
});

test("merge preview rejects readonly, cross-library, and child candidates", async () => {
  async function previewError(options, source, expectedCode) {
    const keeper = mockItem({ key: "KEEP0001", fields: { title: "Keeper" } });
    const { bridge } = loadBridge({ ...options, items: [keeper, source] });
    const token = await authorize(bridge);
    const response = await bridge.handleMergePreview(
      request(
        {
          ...baseRequest(`preview-${expectedCode}`),
          scope: { type: "user" },
          keeper_key: keeper.key,
          source_keys: [source.key],
        },
        { authorization: `Bearer ${token}` },
      ),
    );
    assert.equal(responseJson(response).error.code, expectedCode);
  }

  await previewError(
    { editable: false },
    mockItem({ key: "DUPE0001", fields: { title: "Source" } }),
    "bridge-library-readonly",
  );
  await previewError(
    {},
    mockItem({ key: "DUPE0001", libraryID: 2, fields: { title: "Source" } }),
    "bridge-cross-library",
  );
  await previewError(
    {},
    mockItem({ key: "DUPE0001", regular: false, parentID: 10, fields: {} }),
    "bridge-invalid-child",
  );
});

test("native merge failure maps to transaction error without trashing source", async () => {
  const keeper = mockItem({ key: "KEEP0001", fields: { title: "Keeper", DOI: "" } });
  const source = mockItem({
    key: "DUPE0001",
    itemType: "preprint",
    fields: { title: "Source", DOI: "10.1000/test" },
  });
  const { bridge } = loadBridge({ items: [keeper, source], mergeError: true });
  const token = await authorize(bridge);
  const preview = responseJson(await bridge.handleMergePreview(
    request(
      {
        ...baseRequest("preview-failure"),
        scope: { type: "user" },
        keeper_key: keeper.key,
        source_keys: [source.key],
      },
      { authorization: `Bearer ${token}` },
    ),
  )).data;
  const response = await bridge.handleMergeApply(
    request(
      {
        ...baseRequest("apply-failure"),
        plan_token: preview.plan_token,
        operation_id: "operation-failure",
      },
      { authorization: `Bearer ${token}` },
    ),
  );
  assert.equal(response[0], 500);
  assert.equal(responseJson(response).error.code, "bridge-transaction");
  assert.equal(source.deleted, false);
  assert.equal(keeper.fields.DOI, "");
});

test("shutdown unregisters every bridge endpoint", () => {
  const { bridge, endpoints } = loadBridge();
  bridge.registerEndpoints();
  assert.deepEqual(
    Object.keys(endpoints).sort(),
    [
      "/zot-bridge/v1/auth/revoke",
      "/zot-bridge/v1/health",
      "/zot-bridge/v1/merge/apply",
      "/zot-bridge/v1/merge/preview",
      "/zot-bridge/v1/pair",
      "/zot-bridge/v1/status",
    ],
  );

  bridge.shutdown();
  assert.deepEqual(Object.keys(endpoints), []);
});
