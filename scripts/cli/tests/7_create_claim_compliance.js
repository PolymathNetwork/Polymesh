// Set options as a parameter, environment variable, or rc file.
require = require("esm")(module /*, options*/);
const assert = require("assert");
module.exports = require("../util/init.js");

let { reqImports } = require("../util/init.js");

// Sets the default exit code to fail unless the script runs successfully
process.exitCode = 1;

async function main() {

  const api = await reqImports.createApi();

  let primary_dev_seed = await reqImports.generateRandomKey(api);

  const ticker = await reqImports.generateRandomTicker(api);

  const testEntities = await reqImports.initMain(api);

  let primary_keys = await reqImports.generateKeys( api, 1, primary_dev_seed );

  let issuer_dids = await reqImports.createIdentities( api, primary_keys, testEntities[0] );

  await reqImports.distributePolyBatch( api, primary_keys, reqImports.transfer_amount, testEntities[0] );

  await reqImports.issueTokenPerDid( api, primary_keys, ticker, 1000000, null);

  await createClaimCompliance( api, primary_keys, issuer_dids, ticker );

  if (reqImports.fail_count > 0) {
    console.log("Failed");
  } else {
    console.log("Passed");
    process.exitCode = 0;
  }

  process.exit();
}

async function createClaimCompliance(api, accounts, dids, ticker) {

    assert( ticker.length <= 12, "Ticker cannot be longer than 12 characters");
    let senderConditions = reqImports.senderConditions1(dids[0]);
    let receiverConditions = reqImports.receiverConditions1(dids[0]);
    let transaction = await api.tx.complianceManager.addComplianceRequirement(ticker, [senderConditions], [receiverConditions]);
    let tx = await reqImports.sendTx(accounts[0], transaction);
    if(tx !== -1) reqImports.fail_count--;

}

main().catch(console.error);
