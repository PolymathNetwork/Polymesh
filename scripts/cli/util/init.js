const { ApiPromise, WsProvider, Keyring } = require("@polkadot/api");
const {
  cryptoWaitReady,
  blake2AsHex,
  mnemonicGenerate,
} = require("@polkadot/util-crypto");
const {
  stringToU8a,
  u8aConcat,
  u8aFixLength,
  u8aToHex,
} = require("@polkadot/util");
const BN = require("bn.js");
const assert = require("assert");
const fs = require("fs");
const path = require("path");
const cryptoRandomString = require('crypto-random-string');

let nonces = new Map();

let totalPermissions =
{
  "asset": null,
  "extrinsic": null,
  "portfolio": null
};

let fail_count = 1;
let block_sizes = {};
let block_times = {};

let synced_block = 0;
let synced_block_ts = 0;

// Amount to seed each key with
let transfer_amount = new BN(25000).mul(new BN(10).pow(new BN(6)));

const senderConditions1 = function (trusted_did) {
  return {
      condition_type: {
        IsPresent: {
          Exempted: {
            Identity: trusted_did
          }
        }
      },
      issuers: [{
        issuer: trusted_did,
        trusted_for: {Any: ""}
      }]
    };
};

const receiverConditions1 = senderConditions1;

// Initialization Main is used to generate all entities e.g (Alice, Bob, Dave)
async function initMain(api) {
  let entities = [];

  let alice = await generateEntity(api, "Alice");
  let relay = await generateEntity(api, "relay_1");
  let govCommittee1 = await generateEntity(api, "governance_committee_1");
  let govCommittee2 = await generateEntity(api, "governance_committee_2");

  entities.push(alice);
  entities.push(relay);
  entities.push(govCommittee1);
  entities.push(govCommittee2);

  return entities;
}

const createApi = async function () {
  // Schema path
  const filePath = reqImports.path.join(
    __dirname + "/../../../polymesh_schema.json"
  );
  const { types } = JSON.parse(reqImports.fs.readFileSync(filePath, "utf8"));

  // Start node instance
  const ws_provider = new reqImports.WsProvider(process.env.WS_PROVIDER || "ws://127.0.0.1:9944/");
  const api = await reqImports.ApiPromise.create({
    types,
    provider: ws_provider,
  });
  return api;
};

let generateEntity = async function (api, name) {
  let entity = [];
  await cryptoWaitReady();
  entity = new Keyring({ type: "sr25519" }).addFromUri(`//${name}`, {
    name: `${name}`,
  });
  let entityRawNonce = (await api.query.system.account(entity.address)).nonce;
  let entity_nonce = new BN(entityRawNonce.toString());
  nonces.set(entity.address, entity_nonce);

  return entity;
};

const generateKeys = async function (api, numberOfKeys, keyPrepend) {
  let keys = [];
  await cryptoWaitReady();
  for (let i = 0; i < numberOfKeys; i++) {
    keys.push(
      new Keyring({ type: "sr25519" }).addFromUri(
        "//" + keyPrepend + i.toString(),
        {
          name: i.toString(),
        }
      )
    );
    let accountRawNonce = (await api.query.system.account(keys[i].address))
      .nonce;
    let account_nonce = new BN(accountRawNonce.toString());
    nonces.set(keys[i].address, account_nonce);
  }
  return keys;
};

const generateEntityFromUri = async function (api, uri) {
  await cryptoWaitReady();
  let entity = new Keyring({ type: "sr25519" }).addFromUri(uri);
  let accountRawNonce = (await api.query.system.account(entity.address)).nonce;
  let account_nonce = new BN(accountRawNonce.toString());
  nonces.set(entity.address, account_nonce);
  return entity;
};

const generateRandomEntity = async function (api) {
  let entity = await generateEntityFromUri(api, cryptoRandomString({length: 10}));
  return entity;
}

const generateRandomTicker = async function (api) {
  let ticker = cryptoRandomString({length: 12, type: 'distinguishable'});
  return ticker;
}

const generateRandomKey = async function (api) {
  let ticker = cryptoRandomString({length: 12, type: 'alphanumeric'});
  return ticker;
}

const blockTillPoolEmpty = async function (api) {
  let done = false;
  const unsub = await api.rpc.chain.subscribeNewHeads(async (header) => {
    let pool = await api.rpc.author.pendingExtrinsics();
    if (pool.length == 0) {
      unsub();
      done = true;
      return;
    }
    let last_synced_block = synced_block;
    if (header.number > last_synced_block) {
      for (let i = last_synced_block + 1; i <= header.number; i++) {
        let block_hash = await api.rpc.chain.getBlockHash(i);
        let block = await api.rpc.chain.getBlock(block_hash);
        block_sizes[i] = block["block"]["extrinsics"].length;
        let timestamp_extrinsic = block["block"]["extrinsics"][0];
        let new_block_ts = parseInt(
          JSON.stringify(timestamp_extrinsic["method"].args[0].now)
        );
        block_times[i] = new_block_ts - synced_block_ts;
        synced_block_ts = new_block_ts;
        synced_block = i;
        if (block_sizes[i] > 2) {
          unsub();
          done = true;
        }
      }
    }
  });
  // Should use a mutex here...
  while (!done) {
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
};

// Create a new DID for each of accounts[]
// precondition - accounts all have enough POLY
const createIdentities = async function (api, accounts, alice) {
  return await createIdentitiesWithExpiry(api, accounts, alice, []);
};

const createIdentitiesWithExpiry = async function (
  api,
  accounts,
  alice,
  expiries
) {
  let dids = [];

  for (let i = 0; i < accounts.length; i++) {
    let account_did = await keyToIdentityIds(api, accounts[i].publicKey);

    if(account_did == 0) {
        console.log( `>>>> [Register CDD Claim] acc: ${accounts[i].address}`);
        const transaction = await api.tx.identity.cddRegisterDid(accounts[i].address, []);
        await sendTx(alice, transaction);
    }
    else {
        console.log('Identity Already Linked.');
    }
  }
  await blockTillPoolEmpty(api);

  for (let i = 0; i < accounts.length; i++) {
    const d = await api.query.identity.keyToIdentityIds(accounts[i].publicKey);
    dids.push(d.toHuman());
    console.log(`>>>> [Get DID ] acc: ${accounts[i].address} did: ${dids[i]}`);
  }

  // Add CDD Claim with CDD_ID
  for (let i = 0; i < dids.length; i++) {
    const cdd_id_byte = (i + 1).toString(16).padStart(2, "0");
    const claim = {
      CustomerDueDiligence: `0x00000000000000000000000000000000000000000000000000000000000000${cdd_id_byte}`,
    };
    const expiry = expiries.length == 0 ? null : expiries[i];

    console.log(`>>>> [add CDD Claim] did: ${dids[i]}, claim: ${JSON.stringify(claim)}`);
    await api.tx.identity
      .addClaim(dids[i], claim, expiry)
      .signAndSend(alice, { nonce: nonces.get(alice.address) });

    nonces.set(alice.address, nonces.get(alice.address).addn(1));
  }

  return dids;
};

async function getLastAuth(api, account) {
  const auths = await api.query.identity.authorizations.entries({Account: account.publicKey});
    let last_auth_id = 0;
    for (let i = 0; i < auths.length; i++) {    
      if (auths[i][1].auth_id.toNumber() > last_auth_id) {
        last_auth_id = auths[i][1].auth_id.toNumber();
      }
    }
    return last_auth_id;
}

// Fetches DID that belongs to the Account Key
async function keyToIdentityIds(api, accountKey) {
  let account_did = await api.query.identity.keyToIdentityIds(accountKey);
  return account_did.toHuman();
}

// Sends transfer_amount to accounts[] from alice
async function distributePoly(api, to, amount, from) {
  // Perform the transfers
  let nonceObj = { nonce: nonces.get(from.address) };
  const transaction = api.tx.balances.transfer(to.address, amount);
  await sendTransaction(transaction, from, nonceObj);

  nonces.set(from.address, nonces.get(from.address).addn(1));
}

async function distributePolyBatch(api, to, amount, from) {
  // Perform the transfers
  for (let i = 0; i < to.length; i++) {
    await distributePoly(api, to[i], amount, from);
  }
}

// Attach a secondary key to each DID
async function addSecondaryKeys(api, accounts, dids, secondary_accounts) {
  for (let i = 0; i < accounts.length; i++) {
    // 1. Add Secondary Item to identity.

    let nonceObj = { nonce: nonces.get(accounts[i].address) };
    const transaction = api.tx.identity.addAuthorization({ Account: secondary_accounts[i].publicKey }, { JoinIdentity: reqImports.totalPermissions }, null);
    await sendTransaction(transaction, accounts[i], nonceObj);
    nonces.set(accounts[i].address, nonces.get(accounts[i].address).addn(1));
  }
}

// Authorizes the join of secondary keys to a DID
async function authorizeJoinToIdentities(api, accounts, dids, secondary_accounts) {

  for (let i = 0; i < accounts.length; i++) {
    // 1. Authorize
    const auths = await api.query.identity.authorizations.entries({
      Account: secondary_accounts[i].publicKey
    });
    let last_auth_id = 0;
    for (let i = 0; i < auths.length; i++) {
      if (auths[i][1].auth_id.toNumber() > last_auth_id) {
        last_auth_id = auths[i][1].auth_id.toNumber();
      }
    }

    const transaction = api.tx.identity.joinIdentityAsKey([last_auth_id]);
    await sendTx(secondary_accounts[i], transaction);

    // const unsub = await api.tx.identity
    //   .joinIdentityAsKey([last_auth_id])
    //   .signAndSend(secondary_accounts[i], { nonce: nonces.get(secondary_accounts[i].address) });
    // nonces.set(secondary_accounts[i].address, nonces.get(secondary_accounts[i].address).addn(1));
  }

  return dids;
}

// Creates a token for a did
async function issueTokenPerDid(api, accounts, ticker, amount, fundingRound) {

  assert(ticker.length <= 12, "Ticker cannot be longer than 12 characters");
  let tickerExist = await api.query.asset.tickers(ticker);

  if (tickerExist.owner == 0) {


    const transaction = api.tx.asset.createAsset(
      ticker, ticker, amount, true, 0, [], fundingRound
    );

    await sendTx(accounts[0], transaction);

  } else {
    console.log("ticker exists already");
  }

}

// Returns the asset did
function tickerToDid(ticker) {
  return blake2AsHex(
    u8aConcat(
      stringToU8a("SECURITY_TOKEN:"),
      u8aFixLength(stringToU8a(ticker), 96, true)
    )
  );
}

// Returns a Hex from a String
function stringToHex(ticker) {
  return u8aToHex(
    stringToU8a(ticker)
  );
}

// Creates claim compliance for an asset
async function createClaimCompliance(api, accounts, dids, ticker) {

  assert(ticker.length <= 12, "Ticker cannot be longer than 12 characters");

  let senderConditions = senderConditions1(dids[1]);
  let receiverConditions = receiverConditions1(dids[1]);

  let nonceObj = { nonce: nonces.get(accounts[0].address) };
  const transaction = api.tx.complianceManager.addComplianceRequirement(
    ticker,
    [senderConditions],
    [receiverConditions]
  );
  await sendTransaction(transaction, accounts[0], nonceObj);

  nonces.set(accounts[0].address, nonces.get(accounts[0].address).addn(1));
}

// Adds claim to did
async function addClaimsToDids(
  api,
  accounts,
  did,
  claimType,
  claimValue,
  expiry
) {
  // Receieving Conditions Claim
  let claim = { [claimType]: claimValue };

  let nonceObj = { nonce: nonces.get(accounts[1].address) };
  expiry = expiry == 0 ? null : expiry;
  const transaction = api.tx.identity.addClaim(did, claim, expiry);
  await sendTransaction(transaction, accounts[1], nonceObj);

  nonces.set(accounts[1].address, nonces.get(accounts[1].address).addn(1));
}

const generateStashKeys = async function (api, accounts) {
  let keys = [];
  await cryptoWaitReady();
  for (let i = 0; i < accounts.length; i++) {
    keys.push(
      new Keyring({ type: "sr25519" }).addFromUri(`//${accounts[i]}//stash`, {
        name: `${accounts[i] + "_stash"}`,
      })
    );
    let accountRawNonce = (await api.query.system.account(keys[i].address))
      .nonce;
    let account_nonce = new BN(accountRawNonce.toString());
    nonces.set(keys[i].address, account_nonce);
  }
  return keys;
};

function sendTransaction(transaction, signer, nonceObj) {
  return new Promise((resolve, reject) => {
    const gettingUnsub = transaction.signAndSend(
      signer,
      nonceObj,
      (receipt) => {
        const { status } = receipt;

        if (receipt.isCompleted) {
          /*
           * isCompleted === isFinalized || isError, which means
           * no further updates, so we unsubscribe
           */
          gettingUnsub.then((unsub) => {
            unsub();
          });

          if (receipt.isInBlock) {
            // tx included in a block and finalized
            const failed = receipt.findRecord("system", "ExtrinsicFailed");

            if (failed) {
              // get revert message from event
              let message = "";
              const dispatchError = failed.event.data[0];

              if (dispatchError.isModule) {
                // known error
                const mod = dispatchError.asModule;
                const {
                  section,
                  name,
                  documentation,
                } = mod.registry.findMetaError(
                  new Uint8Array([mod.index.toNumber(), mod.error.toNumber()])
                );

                message = `${section}.${name}: ${documentation.join(" ")}`;
              } else if (dispatchError.isBadOrigin) {
                message = "Bad origin";
              } else if (dispatchError.isCannotLookup) {
                message =
                  "Could not lookup information required to validate the transaction";
              } else {
                message = "Unknown error";
              }

              reject(new Error(message));
            } else {
              resolve(receipt);
            }
          } else if (receipt.isError) {
            reject(new Error("Transaction Aborted"));
          }
        }
      }
    );
  });
}

async function signAndSendTransaction(transaction, signer) {
  let nonceObj = { nonce: nonces.get(signer.address) };
  await sendTransaction(transaction, signer, nonceObj);
  nonces.set(signer.address, nonces.get(signer.address).addn(1));
}

async function generateOffchainKeys(api, keyType) {
  const PHRASE = mnemonicGenerate();
  await cryptoWaitReady();
  const newPair = new Keyring({ type: "sr25519" }).addFromUri(PHRASE);
  await api.rpc.author.insertKey(keyType, PHRASE, u8aToHex(newPair.publicKey));
}

// Creates a Signatory Object
async function signatory(api, entity, signer) {
  let entityKey = entity.publicKey;
  let entityDid = await createIdentities(api, [entity], signer);

  let signatoryObj = {
    Identity: entityDid,
    Account: entityKey,
  };
  return signatoryObj;
}

// Creates a multiSig Key
async function createMultiSig(api, signer, dids, numOfSigners) {
  let nonceObj = { nonce: nonces.get(signer.address) };
  const transaction = api.tx.multiSig.createMultisig(dids, numOfSigners);
  await sendTransaction(transaction, signer, nonceObj);

  nonces.set(signer.address, nonces.get(signer.address).addn(1));
}

async function jumpLightYears() {
  await api.tx.timestamp.set();
}

async function mintingAsset(api, minter, did, ticker) {
  let nonceObj = { nonce: nonces.get(minter.address) };
  const transaction = await api.tx.asset.issue(ticker, 100);
  const result = await sendTransaction(transaction, minter, nonceObj);

  nonces.set(minter.address, nonces.get(minter.address).addn(1));
}

async function sendTx(signer, tx) {
  let nonceObj = { nonce: nonces.get(signer.address) };
  let passed;

  try {
    const result = await sendTransaction(tx, signer, nonceObj);
    passed = result.findRecord("system", "ExtrinsicSuccess");
  } finally {
    nonces.set(signer.address, nonces.get(signer.address).addn(1));
  }

  if (!passed) return -1;

}

async function addComplianceRequirement(api, sender, ticker) {

  let assetCompliance = await api.query.complianceManager.assetCompliances(ticker);

  if (assetCompliance.requirements.length == 0) {
    const transaction = await api.tx.complianceManager.addComplianceRequirement(
      ticker,
      [],
      []
    );

    await sendTx(sender, transaction);
  } else {
    console.log("Asset already has compliance.");
  }
}

async function createVenue(api, sender) {
  let venueCounter = await api.query.settlement.venueCounter();
  let venueDetails = [0];

  const transaction = await api.tx.settlement.createVenue(venueDetails, [
    sender.address,
  ], 0);

  await sendTx(sender, transaction);

  return venueCounter;
}

function getDefaultPortfolio(did) {
  return { "did": did, "kind": "Default" };
}

async function affirmInstruction(api, sender, instructionCounter, did, leg_counts) {
  const transaction = await api.tx.settlement.affirmInstruction(
    instructionCounter,
    [getDefaultPortfolio(did)],
    leg_counts
  );

  await sendTx(sender, transaction);
}

async function withdrawInstruction(api, sender, instructionCounter, did) {
  const transaction = await api.tx.settlement.withdrawInstruction(
    instructionCounter,
    [getDefaultPortfolio(did)]
  );

  await sendTx(sender, transaction);
}

async function rejectInstruction(api, sender, instructionCounter, did) {
  const transaction = await api.tx.settlement.rejectInstruction(
    instructionCounter,
    [getDefaultPortfolio(did)]
  );

  await sendTx(sender, transaction);
}

async function addInstruction(
  api,
  venueCounter,
  sender,
  sender_did,
  receiver_did,
  ticker,
  ticker2,
  amount
) {
  let instructionCounter = await api.query.settlement.instructionCounter();
  let transaction;
  let leg = {
    from: sender_did,
    to: receiver_did,
    asset: ticker,
    amount: amount,
  };

  let leg2 = {
    from: receiver_did,
    to: sender_did,
    asset: ticker2,
    amount: amount,
  };

  if (ticker2 === null || ticker2 === undefined) {
    transaction = await api.tx.settlement.addInstruction(
      venueCounter,
      0,
      null,
      null,
      [leg]
    );
  } else {
    transaction = await api.tx.settlement.addInstruction(
      venueCounter,
      0,
      null,
      null,
      [leg, leg2]
    );
  }

  await sendTx(sender, transaction);

  return instructionCounter;
}

async function addConfidentialInstruction(
  api,
  venueCounter,
  signer,
  sender_did,
  receiver_did,
  mediator_did,
  fromAccountId,
  toAccountId
) {
  let instructionCounter = await api.query.settlement.instructionCounter();

  let ConfidentialLeg = {
    mediator: getDefaultPortfolio(mediator_did),
    from_account_id: fromAccountId,
    to_account_id: toAccountId
  };

    const transaction = await api.tx.settlement.addInstruction(
      venueCounter,
      0,
      null,
      null,
      [
        {
          from: getDefaultPortfolio(sender_did),
          to: getDefaultPortfolio(receiver_did),
          kind: {Confidential: ConfidentialLeg}
        }
      ]
    );

  await sendTx(signer, transaction);

  return instructionCounter;
}

async function claimReceipt(
  api,
  sender,
  sender_did,
  receiver_did,
  ticker,
  amount,
  instructionCounter
) {
  let nonceObj = { nonce: nonces.get(sender.address) };
  let msg = {
    receipt_uid: 0,
    from: sender_did,
    to: receiver_did,
    asset: ticker,
    amount: amount,
  };

  let receiptDetails = {
    receipt_uid: 0,
    leg_id: 0,
    signer: sender.address,
    signature: 1,
  };

  const transaction = await api.tx.settlement.claimReceipt(
    instructionCounter,
    receiptDetails
  );

  await sendTx(sender, transaction);
}

async function getDid(api, account) {
  return await api.query.identity.keyToIdentityIds(account.address);
}

// this object holds the required imports for all the scripts
let reqImports = {
  ApiPromise,
  WsProvider,
  path,
  fs,
  nonces,
  totalPermissions,
  transfer_amount,
  fail_count,
  createApi,
  createIdentities,
  initMain,
  blockTillPoolEmpty,
  generateKeys,
  generateEntityFromUri,
  distributePoly,
  addSecondaryKeys,
  authorizeJoinToIdentities,
  issueTokenPerDid,
  senderConditions1,
  receiverConditions1,
  createClaimCompliance,
  addClaimsToDids,
  tickerToDid,
  sendTransaction,
  generateStashKeys,
  generateEntity,
  signAndSendTransaction,
  distributePolyBatch,
  createIdentitiesWithExpiry,
  generateOffchainKeys,
  signatory,
  createMultiSig,
  u8aToHex,
  keyToIdentityIds,
  mintingAsset,
  sendTx,
  addComplianceRequirement,
  createVenue,
  addInstruction,
  affirmInstruction,
  withdrawInstruction,
  rejectInstruction,
  claimReceipt,
  stringToHex,
  generateRandomEntity,
  generateRandomTicker,
  generateRandomKey,
  getDid,
  getDefaultPortfolio,
  addConfidentialInstruction,
};

export { reqImports };
