// This file is part of the Polymesh distribution (https://github.com/PolymathNetwork/Polymesh).
// Copyright (c) 2020 Polymath

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use crate::IdentityId;
use codec::{Decode, Encode};
use polymesh_primitives_derive::VecU8StrongTyped;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::prelude::Vec;

/// Smart Extension types
#[allow(missing_docs)]
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum SmartExtensionType {
    TransferManager,
    Offerings,
    SmartWallet,
    Custom(Vec<u8>),
}

impl Default for SmartExtensionType {
    fn default() -> Self {
        SmartExtensionType::Custom(b"undefined".to_vec())
    }
}

/// A wrapper for a smart extension name.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
pub struct SmartExtensionName(pub Vec<u8>);

#[derive(Encode, Decode, Default, Clone, PartialEq, Debug)]
/// Smart Extension details when SE instance
/// is attached with asset.
pub struct SmartExtension<AccountId> {
    /// Type of the extension
    pub extension_type: SmartExtensionType,
    /// Name of extension
    pub extension_name: SmartExtensionName,
    /// AccountId of the smart extension
    pub extension_id: AccountId,
    /// Status of the smart extension
    pub is_archive: bool,
}

/// The url string of the SE template.
#[derive(Encode, Decode, Default, Debug, Clone, PartialEq, Eq, VecU8StrongTyped)]
pub struct MetaUrl(pub Vec<u8>);

/// The description string about the SE template.
#[derive(Encode, Decode, Default, Debug, Clone, PartialEq, VecU8StrongTyped)]
pub struct MetaDescription(pub Vec<u8>);

/// The version no. of the SE template.
pub type MetaVersion = u32;

/// Subset of the SE template metadata that is provided by the template owner.
#[derive(Encode, Decode, Default, Debug, Clone, PartialEq)]
pub struct TemplateMetadata<Balance> {
    /// Url that can contain the details about the template
    /// Ex- license, audit report.
    pub url: Option<MetaUrl>,
    /// Type of smart extension template.
    pub se_type: SmartExtensionType,
    /// Fee paid at the time of usage of the SE (A given operation performed).
    pub usage_fee: Balance,
    /// Description about the SE template.
    pub description: MetaDescription,
    /// Version of the template.
    pub version: MetaVersion,
}

/// Data structure that hold all the relevant metadata of the smart extension template.
#[derive(Encode, Decode, Default, Debug, Clone, PartialEq)]
pub struct TemplateDetails<Balance> {
    /// Fee paid at the time on creating new instance form the template.
    pub instantiation_fee: Balance,
    /// Owner of the SE template.
    pub owner: IdentityId,
    /// power button to switch on/off the instantiation from the template
    pub frozen: bool,
}

impl<Balance> TemplateDetails<Balance>
where
    Balance: Clone + Copy,
{
    /// Return the instantiation fee
    pub fn get_instantiation_fee(&self) -> Balance {
        self.instantiation_fee
    }

    /// Check whether the instantiation of the template is allowed or not.
    pub fn is_instantiation_frozen(&self) -> bool {
        self.frozen
    }
}
/// Data structure to hold the details of the smart extension instance.
#[derive(Encode, Decode, Default, Debug, Clone, PartialEq)]
pub struct ExtensionAttributes<Balance> {
    /// Fee that needs to be paid when extension's specific feature get used.
    pub usage_fee: Balance,
    /// Version of extension. It should be compatible with asset version.
    pub version: MetaVersion,
}
