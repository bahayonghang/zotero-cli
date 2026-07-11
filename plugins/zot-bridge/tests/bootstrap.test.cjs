const assert = require("node:assert/strict");
const cryptoModule = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { TextEncoder } = require("node:util");

function loadBridge() {
  const prefs = new Map();
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
        getAll() {
          return [{ libraryType: "user", editable: true, libraryID: 1 }];
        },
      },
      Groups: {
        getByLibraryID() {
          return null;
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
  assert.equal(prefs.size, 0);

  const newStatus = await bridge.handleStatus(
    request(baseRequest("status-after-revoke"), {
      authorization: `Bearer ${token}`,
    }),
  );
  assert.equal(newStatus[0], 401);
});

test("shutdown unregisters every bridge endpoint", () => {
  const { bridge, endpoints } = loadBridge();
  bridge.registerEndpoints();
  assert.deepEqual(
    Object.keys(endpoints).sort(),
    [
      "/zot-bridge/v1/auth/revoke",
      "/zot-bridge/v1/health",
      "/zot-bridge/v1/pair",
      "/zot-bridge/v1/status",
    ],
  );

  bridge.shutdown();
  assert.deepEqual(Object.keys(endpoints), []);
});
