# coreos-metadata

[![Build Status](https://travis-ci.org/coreos/coreos-metadata.svg?branch=master)](https://travis-ci.org/coreos/coreos-metadata)
![minimum rust 1.29](https://img.shields.io/badge/rust-1.29%2B-orange.svg)

This is a small utility, typically used in conjunction with [Ignition][ignition], which reads metadata from a given cloud-provider and applies it to the system.
This can include adding SSH keys and writing cloud-specific attributes into an environment file (e.g. `/run/metadata/coreos`), which can then be consumed by systemd service units via `EnvironmentFile=`.

## Support

The supported cloud providers and their respective metadata are as follows:

  - azure
    - SSH Keys
    - Attributes
      - COREOS_AZURE_IPV4_DYNAMIC
      - COREOS_AZURE_IPV4_VIRTUAL
  - cloudstack-configdrive
    - SSH Keys
    - Attributes
      - COREOS_CLOUDSTACK_AVAILABILITY_ZONE
      - COREOS_CLOUDSTACK_INSTANCE_ID
      - COREOS_CLOUDSTACK_SERVICE_OFFERING
      - COREOS_CLOUDSTACK_CLOUD_IDENTIFIER
      - COREOS_CLOUDSTACK_LOCAL_HOSTNAME
      - COREOS_CLOUDSTACK_VM_ID
  - cloudstack-metadata
    - SSH Keys
    - Attributes
      - COREOS_CLOUDSTACK_AVAILABILITY_ZONE
      - COREOS_CLOUDSTACK_CLOUD_IDENTIFIER
      - COREOS_CLOUDSTACK_HOSTNAME
      - COREOS_CLOUDSTACK_INSTANCE_ID
      - COREOS_CLOUDSTACK_IPV4_LOCAL
      - COREOS_CLOUDSTACK_IPV4_PUBLIC
      - COREOS_CLOUDSTACK_LOCAL_HOSTNAME
      - COREOS_CLOUDSTACK_PUBLIC_HOSTNAME
      - COREOS_CLOUDSTACK_SERVICE_OFFERING
      - COREOS_CLOUDSTACK_VM_ID
  - digitalocean
    - SSH Keys
    - Network Configs
    - Attributes
      - COREOS_DIGITALOCEAN_HOSTNAME
      - COREOS_DIGITALOCEAN_IPV4_ANCHOR_0
      - COREOS_DIGITALOCEAN_IPV4_PUBLIC_0
      - COREOS_DIGITALOCEAN_IPV4_PRIVATE_0
      - COREOS_DIGITALOCEAN_IPV6_PUBLIC_0
      - COREOS_DIGITALOCEAN_IPV6_PRIVATE_0
      - COREOS_DIGITALOCEAN_REGION
  - ec2
    - SSH Keys
    - Attributes
      - COREOS_EC2_HOSTNAME
      - COREOS_EC2_PUBLIC_HOSTNAME
      - COREOS_EC2_IPV4_LOCAL
      - COREOS_EC2_IPV4_PUBLIC
      - COREOS_EC2_AVAILABILITY_ZONE
      - COREOS_EC2_INSTANCE_ID
      - COREOS_EC2_REGION
  - gce
    - SSH Keys
    - Attributes
      - COREOS_GCE_HOSTNAME
      - COREOS_GCE_IP_EXTERNAL_0
      - COREOS_GCE_IP_LOCAL_0
  - hcloud
    - SSH Keys
    - Attributes
      - COREOS_HCLOUD_HOSTNAME
      - COREOS_HCLOUD_IPV4_LOCAL
      - COREOS_HCLOUD_IPV4_PUBLIC
      - COREOS_HCLOUD_INSTANCE_ID
  - openstack-metadata
    - SSH Keys
    - Attributes
      - COREOS_OPENSTACK_HOSTNAME
      - COREOS_OPENSTACK_IPV4_LOCAL
      - COREOS_OPENSTACK_IPV4_PUBLIC
      - COREOS_OPENSTACK_INSTANCE_ID
  - packet
    - SSH Keys
    - Network Configs
    - Attributes
      - COREOS_PACKET_HOSTNAME
      - COREOS_PACKET_IPV4_PUBLIC_0
      - COREOS_PACKET_IPV4_PRIVATE_0
      - COREOS_PACKET_IPV6_PUBLIC_0
  - vagrant-virtualbox
    - Attributes
      - COREOS_VAGRANT_VIRTUALBOX_PRIVATE_IPV4
      - COREOS_VAGRANT_VIRTUALBOX_HOSTNAME

Additionally, some attribute names are reserved for usage by [custom metadata providers][custom-metadata].
These can be safely used by external providers on a platform not supported by coreos-metadata:

  - custom
    - Attributes
      - COREOS_CUSTOM_HOSTNAME
      - COREOS_CUSTOM_PUBLIC_IPV4
      - COREOS_CUSTOM_PRIVATE_IPV4
      - COREOS_CUSTOM_PUBLIC_IPV6
      - COREOS_CUSTOM_PRIVATE_IPV6

[ignition]: https://github.com/coreos/ignition
[custom-metadata]: https://github.com/coreos/container-linux-config-transpiler/blob/v0.8.0/doc/dynamic-data.md#custom-metadata-providers
