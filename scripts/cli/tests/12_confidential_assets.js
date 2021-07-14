// Set options as a parameter, environment variable, or rc file.
require = require("esm")(module /*, options*/);
module.exports = require("../util/init.js");
let { reqImports } = require("../util/init.js");

const {
  create_account,
  create_mediator_account,
  mint_asset,
  create_transaction,
  finalize_transaction,
  justify_transaction,
  Account,
  PubAccount,
  decrypt,
} = require("@polymathnetwork/mercat-nodejs");

const {assert} = require("chai");

async function main() {
  const api = await reqImports.createApi();

  const ticker = await reqImports.generateRandomTicker(api);
  const ticker2 = await reqImports.generateRandomTicker(api);
  const tickerHex = Buffer.from(ticker, 'utf8').toString('hex');
  const testEntities = await reqImports.initMain(api);

  let alice = testEntities[0];
  let bob = await reqImports.generateRandomEntity(api);
  let charlie = await reqImports.generateRandomEntity(api);
  let dave = await reqImports.generateRandomEntity(api);

  let alice_did = await reqImports.keyToIdentityIds(api, alice.publicKey);

  let dids = await reqImports.createIdentities(api, [bob, charlie, dave], alice);
  let bob_did = dids[0];
  let charlie_did = dids[1];
  let dave_did = dids[2];

  await reqImports.distributePolyBatch(
    api,
    [bob, charlie, dave],
    reqImports.transfer_amount * 2,
    alice
  );

  console.log("Alice: ", alice_did);
  console.log("Bob: ", bob_did);
  console.log("Charlie: ", charlie_did);
  console.log("Dave: ", dave_did);


  // Dave creates Confidential Assets
  console.log("-----------> Creating confidential assets.");
  await createConfidentialAsset(api, ticker, dave);
  await createConfidentialAsset(api, ticker2, dave);

  let tickers =  await api.query.confidentialAsset.confidentialTickers();
  let tickersHexList = createSubStr(tickers);

  // Dave and Bob create their Mercat account locally and submit the proof to the chain
  console.log("-----------> Creating Dave and Bob's mercat accounts.");
  const daveMercatInfo = create_account(tickersHexList, tickerHex);
  const bobMercatInfo = create_account(tickersHexList, tickerHex);

  // Validate Dave and Bob's Mercat Accounts
  console.log("-----------> Submitting dave mercat account proofs.");
  await validateMercatAccount(api, dave, daveMercatInfo.account_tx);
  console.log("-----------> Submitting bob mercat account proofs.");
  await validateMercatAccount(api, bob, bobMercatInfo.account_tx);

  // Charlie creates his mediator Mercat Account
  console.log("-----------> Creating Charlie's account.");
  const charlieMercatAccount = createMercatMediatorAccount();
  const charliePublicKey = charlieMercatAccount.public_key;
  const charlieAccount = charlieMercatAccount.secret_account;

  // Validate Charlie's Mercat Account
  console.log("-----------> Submitting Charlie's account.");
  await addMediatorMercatAccount(api, charlie, charliePublicKey);

  await displayBalance(api, dave_did, daveMercatInfo, "Dave initial balance");
  await displayBalance(api, bob_did, bobMercatInfo, "Bob initial balance");

  // Mint Tokens
  console.log("-----------> Minting assets.");
  await mintTokens(api, dave, ticker, 1000, daveMercatInfo);

  await displayBalance(api, dave_did, daveMercatInfo, "Dave balance after minting");

  // Create Venue
  console.log("-----------> Creating venue.");
  const venueCounter = await reqImports.createVenue(api, charlie);

  // Create Confidential Instruction
  console.log("-----------> Creating confidential instruction.");
  const instructionCounter = await reqImports.addConfidentialInstruction(
    api,
    venueCounter,
    charlie,
    dave_did,
    bob_did,
    charlie_did,
    daveMercatInfo.account_id,
    bobMercatInfo.account_id
  );

  console.log("-----------> Initializing confidential transaction.");
  let bobPublicAccount = new PubAccount(bobMercatInfo.account_id, bobMercatInfo.public_key);
  let daveEncryptedBalance = await getEncryptedBalance(api, dave_did, daveMercatInfo.account_id);
  const initTransactionProof = createTransaction(
    100,
    daveMercatInfo,
    bobPublicAccount,
    charliePublicKey,
    daveEncryptedBalance
  );

  console.log("-----------> Submitting initial confidential transaction proof.");
  await affirmConfidentialInstruction(api, instructionCounter, {InitializedTransfer: initTransactionProof}, dave, dave_did);

  console.log("-----------> Finalizing confidential transaction.");
  const finalizeTransactionProof = finalizeTransaction(
    100,
    initTransactionProof,
    bobMercatInfo,
  );

  console.log("-----------> Submitting finalize confidential transaction proof.");
  bobPublicAccount = new PubAccount(bobMercatInfo.account_id, bobMercatInfo.public_key);
  await affirmConfidentialInstruction(api, instructionCounter, {FinalizedTransfer: finalizeTransactionProof}, bob, bob_did);

  const davePublicAccount = new PubAccount(daveMercatInfo.account_id, daveMercatInfo.public_key);

  console.log("-----------> Justifying confidential transaction.");
  const justifiedTransactionProof = (justify_transaction(finalizeTransactionProof, charlieAccount, davePublicAccount, daveEncryptedBalance, bobPublicAccount, tickerHex)).justified_tx;

  console.log("-----------> Submitting justify confidential transaction proof.");
  await affirmConfidentialInstruction(api, instructionCounter, {JustifiedTransfer: justifiedTransactionProof}, charlie, charlie_did);

  await displayBalance(api, dave_did, daveMercatInfo, "Dave balance after giving tokens to Bob");
  await displayBalance(api, bob_did, bobMercatInfo, "Bob balance after getting tokens from Dave");

  daveEncryptedBalance = await getEncryptedBalance(api, dave_did, daveMercatInfo.account_id);
  const bobEncryptedBalance = await getEncryptedBalance(api, bob_did, bobMercatInfo.account_id);

  assert.equal(decryptBalances(daveEncryptedBalance, daveMercatInfo), 900);
  assert.equal(decryptBalances(bobEncryptedBalance, bobMercatInfo), 100);

  if (reqImports.fail_count > 0) {
    console.log("Failed");
  } else {
    console.log("Passed");
    process.exitCode = 0;
  }

  process.exit();
}

function createSubStr(tickers) {
  let newArray = [];
  for(let i =0; i < tickers.length; i++) 
    newArray.push(String(tickers[i]).substr(2)); 
  return newArray;
}

function finalizeTransaction(amount, initializeProof, receiverMercatAccountInfo) {
  const receiverPublicAccount = new PubAccount(receiverMercatAccountInfo.account_id, receiverMercatAccountInfo.public_key);
  const receiverAccount = new Account(receiverMercatAccountInfo.secret_account, receiverPublicAccount);

  let tx = finalize_transaction(amount, initializeProof, receiverAccount);

  return tx.finalized_tx;
}

async function affirmConfidentialInstruction(api, instruction_id, proof, signer, signer_did) {
    const portfolio = reqImports.getDefaultPortfolio(signer_did);
    const transaction = await api.tx.settlement.affirmConfidentialInstruction(instruction_id, proof, [portfolio], 1);
    let tx = await reqImports.sendTx(signer, transaction);
    if(tx !== -1) reqImports.fail_count--;
}

function createTransaction(amount, senderMercatAccountInfo, receiverPubAccount, mediatorPublicKey, encrypted_pending_balance) {
  const senderPublicAccount = new PubAccount(senderMercatAccountInfo.account_id, senderMercatAccountInfo.public_key);
  const senderAccount = new Account(senderMercatAccountInfo.secret_account, senderPublicAccount);

  let tx = create_transaction(amount, senderAccount, encrypted_pending_balance, receiverPubAccount, mediatorPublicKey);

  return tx.init_tx;
}

async function displayBalance(api, did, mercatAccountInfo, message) {
    // Get encrypted balance
    const accountEncryptedBalance = await getEncryptedBalance(api, did, mercatAccountInfo.account_id);
    // Decrypt balance
    const accountBalance = decryptBalances(accountEncryptedBalance, mercatAccountInfo);
    console.log(`${message}: ${accountBalance}`);

    return accountEncryptedBalance;
}

async function mintTokens(api, signer, ticker, amount, mercatAccountInfo) {
  const publicAccount = new PubAccount(mercatAccountInfo.account_id,  mercatAccountInfo.public_key);
  const account = new Account(mercatAccountInfo.secret_account, publicAccount);
  const mintTxInfo = new mint_asset(amount, account);
  const mintProof = mintTxInfo.asset_tx;

  const transaction = await api.tx.confidentialAsset.mintConfidentialAsset(ticker, amount, mintProof);
  let tx = await reqImports.sendTx(signer, transaction);
  if(tx !== -1) reqImports.fail_count--;
}

async function getEncryptedBalance(api, did, mercatAccountID){
    return String(await api.query.confidentialAsset.mercatAccountBalance(did, mercatAccountID));
}

function decryptBalances(encryptedBalance, mercatAccountInfo) {
  const publicAccount = new PubAccount(mercatAccountInfo.account_id,  mercatAccountInfo.public_key);
  const account = new Account(mercatAccountInfo.secret_account, publicAccount);

  return decrypt(encryptedBalance, account);
}

async function addMediatorMercatAccount(api, signer, public_key) {
    const transaction = await api.tx.confidentialAsset.addMediatorMercatAccount(public_key);

    let tx = await reqImports.sendTx(signer, transaction);
    if(tx !== -1) reqImports.fail_count--;
}

async function validateMercatAccount(api, signer, proof) {
    const transaction = await api.tx.confidentialAsset.validateMercatAccount(proof);

    let tx = await reqImports.sendTx(signer, transaction);
    if(tx !== -1) reqImports.fail_count--;
}

function createMercatMediatorAccount() {
  const charlieMercatInfo = create_mediator_account();

  return charlieMercatInfo;
}

async function createConfidentialAsset(api, ticker, signer) {

    const transaction = await api.tx.confidentialAsset.createConfidentialAsset(
        ticker,
        ticker,
        true,
        0,
        [],
        null
      );

      let tx = await reqImports.sendTx(signer, transaction);
      if(tx !== -1) reqImports.fail_count--;
}

main().catch(console.error);
