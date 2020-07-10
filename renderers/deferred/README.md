# Deferred

![deferred](../../docs/deferred.png)

A deferred renderer using wgpu-rs. Supports the following:
- PBR Materials using Disney's BSDF
- Shadow maps for directional lights and spot lights

# Issues
- Currently waiting for wgpu-rs to properly support cube maps. These are required for implementing point light shadow maps.
- SSAO still shows artifacts and is a WIP.