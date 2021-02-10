use crate::types::{Claim1stKey, Claim2ndKey};

use confidential_identity::mocked::make_investor_uid;
use polymesh_primitives::{CddId, Claim, ClaimType, IdentityClaim, IdentityId};

/// Migrate claim
pub fn migrate_claim(
    k1: Claim1stKey,
    _k2: Claim2ndKey,
    id_claim: IdentityClaim,
) -> Option<IdentityClaim> {
    match &k1.claim_type {
        ClaimType::CustomerDueDiligence => migrate_cdd_claim(k1.target, id_claim),
        _ => Some(id_claim),
    }
}

/// CDD claims are going to be mocked, where the Investor UID is the hash of its `IdentityId`.
fn migrate_cdd_claim(target: IdentityId, mut id_claim: IdentityClaim) -> Option<IdentityClaim> {
    let uid = make_investor_uid(target.as_bytes()).into();
    let cdd_id = CddId::new(target, uid);

    id_claim.claim = Claim::CustomerDueDiligence(cdd_id);
    Some(id_claim)
}
