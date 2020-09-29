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

use crate::errors;
use crate::providers;
use crate::providers::aliyun::AliyunProvider;
use crate::providers::aws::AwsProvider;
use crate::providers::azure::Azure;
use crate::providers::cloudstack::configdrive::ConfigDrive;
use crate::providers::cloudstack::network::CloudstackNetwork;
use crate::providers::digitalocean::DigitalOceanProvider;
use crate::providers::exoscale::ExoscaleProvider;
use crate::providers::gcp::GcpProvider;
use crate::providers::ibmcloud::IBMGen2Provider;
use crate::providers::ibmcloud_classic::IBMClassicProvider;
use crate::providers::openstack;
use crate::providers::openstack::network::OpenstackProviderNetwork;
use crate::providers::packet::PacketProvider;
use crate::providers::qemu::QemuProvider;
#[cfg(feature = "cl-legacy")]
use crate::providers::vagrant_virtualbox::VagrantVirtualboxProvider;
use crate::providers::vmware::VmwareProvider;
use crate::providers::vultr::VultrProvider;

macro_rules! box_result {
    ($exp:expr) => {
        Ok(Box::new($exp))
    };
}

/// Fetch metadata for the given provider.
///
/// This is the generic, top-level function to fetch provider metadata.
/// The configured provider is passed in and this function dispatches the call
/// to the provider-specific fetch logic.
pub fn fetch_metadata(provider: &str) -> errors::Result<Box<dyn providers::MetadataProvider>> {
    match provider {
        "aliyun" => box_result!(AliyunProvider::try_new()?),
        #[cfg(not(feature = "cl-legacy"))]
        "aws" => box_result!(AwsProvider::try_new()?),
        "azure" => box_result!(Azure::try_new()?),
        "cloudstack-metadata" => box_result!(CloudstackNetwork::try_new()?),
        "cloudstack-configdrive" => box_result!(ConfigDrive::try_new()?),
        "digitalocean" => box_result!(DigitalOceanProvider::try_new()?),
        "exoscale" => box_result!(ExoscaleProvider::try_new()?),
        #[cfg(feature = "cl-legacy")]
        "ec2" => box_result!(AwsProvider::try_new()?),
        #[cfg(feature = "cl-legacy")]
        "gce" => box_result!(GcpProvider::try_new()?),
        #[cfg(not(feature = "cl-legacy"))]
        "gcp" => box_result!(GcpProvider::try_new()?),
        // IBM Cloud - VPC Generation 2.
        "ibmcloud" => box_result!(IBMGen2Provider::try_new()?),
        // IBM Cloud - Classic infrastructure.
        "ibmcloud-classic" => box_result!(IBMClassicProvider::try_new()?),
        "openstack" => openstack::try_config_drive_else_network(),
        "openstack-metadata" => box_result!(OpenstackProviderNetwork::try_new()?),
        "packet" => box_result!(PacketProvider::try_new()?),
        "qemu" => box_result!(QemuProvider::try_new()?),
        #[cfg(feature = "cl-legacy")]
        "vagrant-virtualbox" => box_result!(VagrantVirtualboxProvider::new()),
        "vmware" => box_result!(VmwareProvider::try_new()?),
        "vultr" => box_result!(VultrProvider::try_new()?),
        _ => Err(errors::ErrorKind::UnknownProvider(provider.to_owned()).into()),
    }
}
