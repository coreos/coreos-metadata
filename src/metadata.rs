// Copyright 2017 CoreOS, Inc.
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

use errors;
use providers;
use providers::azure::Azure;
use providers::cloudstack::configdrive::ConfigDrive;
use providers::cloudstack::network::CloudstackNetwork;
use providers::digitalocean::DigitalOceanProvider;
use providers::ec2::Ec2Provider;
use providers::gce::GceProvider;
use providers::hcloud::HetznerCloudProvider;
use providers::openstack::network::OpenstackProvider;
use providers::packet::PacketProvider;
use providers::vagrant_virtualbox::VagrantVirtualboxProvider;

macro_rules! box_result {
    ($exp:expr) => {
        Ok(Box::new($exp))
    };
}

/// `fetch_metadata` is the generic, top-level function that is used by the main
/// function to fetch metadata. The configured provider is passed in and this
/// function dispatches the call to the correct provider-specific fetch function
pub fn fetch_metadata(provider: &str) -> errors::Result<Box<providers::MetadataProvider>> {
    match provider {
        "azure" => box_result!(Azure::try_new()?),
        "cloudstack-metadata" => box_result!(CloudstackNetwork::try_new()?),
        "cloudstack-configdrive" => box_result!(ConfigDrive::try_new()?),
        "digitalocean" => box_result!(DigitalOceanProvider::try_new()?),
        "ec2" => box_result!(Ec2Provider::try_new()?),
        "gce" => box_result!(GceProvider::try_new()?),
        "hcloud" => box_result!(HetznerCloudProvider::try_new()?),
        "openstack-metadata" => box_result!(OpenstackProvider::try_new()?),
        "packet" => box_result!(PacketProvider::try_new()?),
        "vagrant-virtualbox" => box_result!(VagrantVirtualboxProvider::new()),
        _ => Err(errors::ErrorKind::UnknownProvider(provider.to_owned()).into()),
    }
}
