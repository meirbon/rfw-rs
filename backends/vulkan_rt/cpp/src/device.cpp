#include "device.h"

namespace vkh
{

char ascii_tolower(char in)
{
	if (in <= 'Z' && in >= 'A')
		return in - ('Z' - 'z');
	return in;
}

vk::PhysicalDevice pickPhysicalDevice(vk::Instance instance, const char *vendorName)
{
	std::string vendor = vendorName;
	std::transform(vendor.begin(), vendor.end(), vendor.begin(), ascii_tolower);

	std::vector<vk::PhysicalDevice> physicalDevices = instance.enumeratePhysicalDevices();
	const size_t index =
		std::distance(physicalDevices.begin(), std::find_if(physicalDevices.begin(), physicalDevices.end(),
															[&vendor](const vk::PhysicalDevice &physicalDevice) {
																const auto properties = physicalDevice.getProperties();
																std::string deviceName = properties.deviceName;
																std::transform(deviceName.begin(), deviceName.end(),
																			   deviceName.begin(), ascii_tolower);
																return deviceName.find(vendor) != std::string::npos;
															}));

	if (index >= physicalDevices.size())
		return nullptr;
	return physicalDevices[index];
}

std::set<uint32_t> findQueueFamilyIndices(vk::PhysicalDevice physicalDevice, vk::SurfaceKHR surface,
										  uint32_t *graphicsQueue, uint32_t *presentQueue)
{
	const std::vector<vk::QueueFamilyProperties> queueFamilyProperties = physicalDevice.getQueueFamilyProperties();
	const uint32_t graphicsQueueFamilyIndex = static_cast<uint32_t>(std::distance(
		queueFamilyProperties.begin(), std::find_if(queueFamilyProperties.begin(), queueFamilyProperties.end(),
													[](vk::QueueFamilyProperties const &qfp) {
														return qfp.queueFlags & vk::QueueFlagBits::eGraphics;
													})));

	uint32_t presentQueueFamilyIndex = 0u;
	for (size_t i = 0; i < queueFamilyProperties.size(); i++)
	{
		if (physicalDevice.getSurfaceSupportKHR(static_cast<uint32_t>(i), surface))
			presentQueueFamilyIndex = static_cast<uint32_t>(i);
	}

	if (graphicsQueue)
		*graphicsQueue = graphicsQueueFamilyIndex;
	if (presentQueue)
		*presentQueue = presentQueueFamilyIndex;

	return {graphicsQueueFamilyIndex, presentQueueFamilyIndex};
}
} // namespace vkh