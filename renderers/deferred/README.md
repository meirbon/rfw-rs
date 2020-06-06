# Deferred

A deferred renderer using wgpu-rs. Supports the following:
- PBR Materials using Disney's BSDF
- Shadow maps for directional lights and spot lights

# Issues
- Currently waiting for wgpu-rs to properly support cube maps. These are required for implementing point light shadow maps.
- Support for area lights needs to be added to scene before it can be used here.
- SSAO still shows artifacts and is a WIP.