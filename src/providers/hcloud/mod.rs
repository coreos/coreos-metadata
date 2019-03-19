// Copyright 2018 CoreOS, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Hetzner Cloud metadata fetcher
//!

use std::collections::HashMap;

use openssh_keys::PublicKey;
use update_ssh_keys::AuthorizedKeyEntry;

use errors::*;
use network;
use providers::MetadataProvider;
use retry;
use serde_json;

const URL: &str = "http://169.254.169.254/2009-04-04";

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct InstanceIdDoc {
    region: String,
}

#[derive(Clone, Debug)]
pub struct HetznerCloudProvider {
    client: retry::Client,
}

impl HetznerCloudProvider {
    pub fn try_new() -> Result<HetznerCloudProvider> {
        let client = retry::Client::try_new()?
            .return_on_404(true);

        Ok(HetznerCloudProvider { client })
    }

    fn endpoint_for(key: &str) -> String {
        format!("{}/{}", URL, key)
    }
}

impl MetadataProvider for HetznerCloudProvider {
    fn attributes(&self) -> Result<HashMap<String, String>> {
        let mut out = HashMap::with_capacity(4);

        let add_value = |map: &mut HashMap<_, _>, key: &str, name| -> Result<()> {
            let value = self.client.get(retry::Raw, HetznerCloudProvider::endpoint_for(name)).send()?;

            if let Some(value) = value {
                map.insert(key.to_string(), value);
            }

            Ok(())
        };

        add_value(&mut out, "HCLOUD_INSTANCE_ID", "meta-data/instance-id")?;
        add_value(&mut out, "HCLOUD_IPV4_LOCAL", "meta-data/local-ipv4")?;
        add_value(&mut out, "HCLOUD_IPV4_PUBLIC", "meta-data/public-ipv4")?;
        add_value(&mut out, "HCLOUD_HOSTNAME", "meta-data/hostname")?;

        Ok(out)
    }

    fn hostname(&self) -> Result<Option<String>> {
        self.client.get(retry::Raw, HetznerCloudProvider::endpoint_for("meta-data/hostname")).send()
    }

    fn ssh_keys(&self) -> Result<Vec<AuthorizedKeyEntry>> {
        let raw_keys: Option<String> = self.client
            .get(retry::Raw, HetznerCloudProvider::endpoint_for("meta-data/public-keys"))
            .send()?;

        if let Some(raw_keys) = raw_keys {
            let keys: Vec<String> = serde_json::from_str(&raw_keys).unwrap();
            let mut out = Vec::new();

            for key in keys {
                let key = PublicKey::parse(&key)?;
                out.push(AuthorizedKeyEntry::Valid{key});
            }

            Ok(out)
        } else {
            Ok(vec![])
        }
    }

    fn networks(&self) -> Result<Vec<network::Interface>> {
        Ok(vec![])
    }

    fn network_devices(&self) -> Result<Vec<network::Device>> {
        Ok(vec![])
    }
}
