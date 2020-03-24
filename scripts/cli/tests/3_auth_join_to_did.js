// Set options as a parameter, environment variable, or rc file.
require = require("esm")(module /*, options*/);
module.exports = require("../util/init.js");

let { reqImports } = require("../util/init.js");

async function main() {
  // Schema path
  const filePath = reqImports["path"].join(__dirname + "/../../../polymesh_schema.json");
  const customTypes = JSON.parse(reqImports["fs"].readFileSync(filePath, "utf8"));

  // Start node instance
  const ws_provider = new reqImports["WsProvider"]("ws://127.0.0.1:9944/");
  const api = await reqImports["ApiPromise"].create({
    types: customTypes,
    provider: ws_provider
  });

  const testEntities = await reqImports["initMain"](api);

  let master_keys = await reqImports["generateKeys"](api,5, "master");

  let signing_keys = await reqImports["generateKeys"](api, 5, "signing");

  await reqImports["createIdentities"](api, testEntities);

  await reqImports["distributePoly"]( api, master_keys.concat(signing_keys), reqImports["transfer_amount"], testEntities[0] );

  await reqImports["blockTillPoolEmpty"](api);

  let issuer_dids = await reqImports["createIdentities"](api, master_keys);

  await reqImports["addSigningKeys"]( api, master_keys, issuer_dids, signing_keys );

  await authorizeJoinToIdentities( api, master_keys, issuer_dids, signing_keys);

  await reqImports["blockTillPoolEmpty"](api);

  await new Promise(resolve => setTimeout(resolve, 3000));

  if (reqImports["fail_count"] > 0) {
    console.log("Failed");
    process.exitCode = 1;
  } else {
    console.log("Passed");
  }

  process.exit();
}

// Authorizes the join of signing keys to a DID
async function authorizeJoinToIdentities(api, accounts, dids, signing_accounts) {

  for (let i = 0; i < accounts.length; i++) {
    // 1. Authorize
    const auths = await api.query.identity.authorizations.entries({AccountKey: signing_accounts[i].publicKey});
    let last_auth_id = 0;
    for (let i = 0; i < auths.length; i++) {
      if (auths[i][1].auth_id.toNumber() > last_auth_id) {
        last_auth_id = auths[i][1].auth_id.toNumber()
      }
    }
    const unsub = await api.tx.identity
    .joinIdentityAsKey([last_auth_id])
    .signAndSend(signing_accounts[i],
      { nonce: reqImports["nonces"].get(signing_accounts[i].address) },
      ({ events = [], status }) => {
      if (status.isFinalized) {
        reqImports["fail_count"] = reqImports["callback"](status, events, "identity", "NewSigningItems", reqImports["fail_count"]);
        unsub();
      }
    });
  }

  return dids;
}

main().catch(console.error);
