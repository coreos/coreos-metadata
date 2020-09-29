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

#![allow(deprecated)]

use error_chain::error_chain;

error_chain! {
    links {
        AuthorizedKeys(::update_ssh_keys::errors::Error, ::update_ssh_keys::errors::ErrorKind) #[cfg(feature = "cl-legacy")];
        PublicKey(::openssh_keys::errors::Error, ::openssh_keys::errors::ErrorKind);
    }
    foreign_links {
        Base64Decode(::base64::DecodeError);
        Clap(clap::Error);
        HeaderValue(reqwest::header::InvalidHeaderValue);
        Io(::std::io::Error);
        IpNetwork(ipnetwork::IpNetworkError);
        Json(serde_json::Error);
        Log(::slog::Error);
        MacAddr(pnet_base::ParseMacAddrErr);
        OvirtGuestAgent(tokio_oga::OgaError);
        OpensslStack(::openssl::error::ErrorStack);
        Reqwest(::reqwest::Error);
        VmwBackdoor(vmw_backdoor::VmwError);
        XmlDeserialize(::serde_xml_rs::Error);
    }
    errors {
        UnknownProvider(p: String) {
            description("unknown provider")
            display("unknown provider '{}'", p)
        }
    }
}
