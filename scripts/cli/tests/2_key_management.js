// Set options as a parameter, environment variable, or rc file.
require = require("esm")(module /*, options*/);
module.exports = require("../util/init.js");

let { reqImports } = require("../util/init.js");

// Sets the default exit code to fail unless the script runs successfully
process.exitCode = 1;

async function main() {
 
  const api = await reqImports.createApi();

  const testEntities = await reqImports.initMain(api);

  let master_keys = await reqImports.generateKeys(api,5, "master");

  let signing_keys = await reqImports.generateKeys(api, 5, "signing");

  await reqImports.createIdentities(api, testEntities, testEntities[0]);

  await reqImports.distributePoly( api, master_keys.concat(signing_keys), reqImports.transfer_amount, testEntities[0] );

  await reqImports.blockTillPoolEmpty(api);

  let issuer_dids = await reqImports.createIdentities(api, master_keys, testEntities[0]);

  await addSigningKeys( api, master_keys, issuer_dids, signing_keys );

  await reqImports.blockTillPoolEmpty(api);

  await new Promise(resolve => setTimeout(resolve, 3000));

  if (reqImports.fail_count > 0) {
    console.log("Failed");
  } else {
    console.log("Passed");
    process.exitCode = 0;
  }

  process.exit();
}

// Attach a signing key to each DID
async function addSigningKeys( api, accounts, dids, signing_accounts ) {

  for (let i = 0; i < accounts.length; i++) {
    // 1. Add Signing Item to identity.
    const unsub = await api.tx.identity
      .addAuthorizationAsKey({AccountKey: signing_accounts[i].publicKey}, {JoinIdentity: dids[i]}, 0)
      .signAndSend(
        accounts[i],
        { nonce: reqImports.nonces.get(accounts[i].address) },
        ({ events = [], status }) => {

          if (status.isFinalized) {
            // Loop through Vec<EventRecord> to display all events
            events.forEach(({ phase, event: { data, method, section } }) => {
              if ( section === "system" && method === "ExtrinsicSuccess" )  reqImports.fail_count--;
            });
            unsub();
          }
        }

      );

    reqImports.nonces.set(accounts[i].address, reqImports.nonces.get(accounts[i].address).addn(1));
  }
}

main().catch(console.error);
