#if MACOS || IOS
#include "create_metal_layer.h"
#import <MetalKit/MetalKit.h>
#endif

#if MACOS
void *vulkan_create_metal_layer(void * /*ns_view*/, void *ns_window)
{
	NSWindow *window = reinterpret_cast<NSWindow *>(ns_window);
	window.contentView.wantsLayer = YES;
	CAMetalLayer *layer = [CAMetalLayer new];
	window.contentView.layer = layer;
	return layer;
}
#endif

#if IOS
void *vulkan_create_metal_layer(void * /*ui_view*/, void *ui_window)
{
	UIWindow *window = reinterpret_cast<UIWindow *>(ui_window);
	CAMetalLayer *layer = [CAMetalLayer new];
	window.contentView.wantsLayer = YES;
	window.contentView.layer = layer;
	return layer;
}
#endif