//! Vulkan ProcAddr Dispatcher (vkGetInstanceProcAddr / vkGetDeviceProcAddr Interceptors)
//! Resolves requested Vulkan function names to typed Win64 ABI thunks in wrappers.rs.
//!
//! CRITICAL SAFETY INVARIANT: NEVER return a raw native System V pointer to the PE64 guest.
//! All returned function pointers MUST point to `extern "win64"` thunk wrappers.
//! If no thunk exists for a requested command, return NULL (with a diagnostic log).

#![allow(non_snake_case)]

use super::extensions::{rine_vkCreateInstance, rine_vkEnumerateInstanceExtensionProperties};
use super::loader::get_native_proc_addr;
use super::surface::{
    rine_vkCreateWin32SurfaceKHR, rine_vkGetPhysicalDeviceWin32PresentationSupportKHR,
};
use super::types::*;
use super::wrappers::*;
use std::collections::HashSet;
use std::ffi::{c_char, c_void, CStr};
use std::sync::Mutex;

/// Resolve a function from native vkGetInstanceProcAddr.
/// For internal use only — never expose the result directly to PE64 guest code.
pub unsafe fn get_native_instance_proc_addr(
    instance: NativeVkInstance,
    fn_name: &str,
) -> Option<usize> {
    let sym = get_native_proc_addr("vkGetInstanceProcAddr")?;
    type VkGetInstanceProcAddrFn =
        unsafe extern "C" fn(NativeVkInstance, *const c_char) -> *const c_void;
    let native_get_proc_addr: VkGetInstanceProcAddrFn = std::mem::transmute(sym);
    let c_name = std::ffi::CString::new(fn_name).ok()?;
    let ptr = native_get_proc_addr(instance, c_name.as_ptr());
    if ptr.is_null() {
        None
    } else {
        Some(ptr as usize)
    }
}

/// Thread-safe sets of already-logged command names for GIPA and GDPA diagnostics
static LOGGED_GIPA: std::sync::LazyLock<Mutex<HashSet<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashSet::new()));
static LOGGED_GDPA: std::sync::LazyLock<Mutex<HashSet<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashSet::new()));

fn log_gipa_once(name: &str, wrapper: *const c_void) {
    if let Ok(mut set) = LOGGED_GIPA.lock() {
        if set.insert(name.to_string()) {
            let supported = !wrapper.is_null();
            println!(
                "[VK GIPA] name={} wrapper={:p} supported={}",
                name, wrapper, supported
            );
        }
    }
}

fn log_gdpa_once(name: &str, wrapper: *const c_void) {
    if let Ok(mut set) = LOGGED_GDPA.lock() {
        if set.insert(name.to_string()) {
            let supported = !wrapper.is_null();
            println!(
                "[VK GDPA] name={} wrapper={:p} supported={}",
                name, wrapper, supported
            );
        }
    }
}

/// Device-level dispatch registry: matches commands taking VkDevice, VkQueue, or VkCommandBuffer.
/// Global and instance commands (like vkCreateInstance or vkEnumeratePhysicalDevices) MUST return None here.
fn lookup_device_thunk(name_bytes: &[u8]) -> Option<*const c_void> {
    let ptr: *const c_void = match name_bytes {
        b"vkGetDeviceProcAddr" => rine_vkGetDeviceProcAddr as *const c_void,

        // Device lifecycle
        b"vkDestroyDevice" => guest_vkDestroyDevice as *const c_void,
        b"vkGetDeviceQueue" => guest_vkGetDeviceQueue as *const c_void,
        b"vkGetDeviceQueue2" => guest_vkGetDeviceQueue2 as *const c_void,

        // Swapchain
        b"vkCreateSwapchainKHR" => guest_vkCreateSwapchainKHR as *const c_void,
        b"vkDestroySwapchainKHR" => guest_vkDestroySwapchainKHR as *const c_void,
        b"vkGetSwapchainImagesKHR" => guest_vkGetSwapchainImagesKHR as *const c_void,
        b"vkAcquireNextImageKHR" => guest_vkAcquireNextImageKHR as *const c_void,
        b"vkAcquireNextImage2KHR" => guest_vkAcquireNextImage2KHR as *const c_void,

        // Queue
        b"vkQueueSubmit" => guest_vkQueueSubmit as *const c_void,
        b"vkQueueSubmit2" | b"vkQueueSubmit2KHR" => guest_vkQueueSubmit2 as *const c_void,
        b"vkQueuePresentKHR" => guest_vkQueuePresentKHR as *const c_void,
        b"vkDeviceWaitIdle" => guest_vkDeviceWaitIdle as *const c_void,
        b"vkQueueWaitIdle" => guest_vkQueueWaitIdle as *const c_void,
        b"vkQueueBindSparse" => guest_vkQueueBindSparse as *const c_void,

        // Sync
        b"vkCreateSemaphore" => guest_vkCreateSemaphore as *const c_void,
        b"vkDestroySemaphore" => guest_vkDestroySemaphore as *const c_void,
        b"vkCreateFence" => guest_vkCreateFence as *const c_void,
        b"vkDestroyFence" => guest_vkDestroyFence as *const c_void,
        b"vkWaitForFences" => guest_vkWaitForFences as *const c_void,
        b"vkResetFences" => guest_vkResetFences as *const c_void,
        b"vkGetFenceStatus" => guest_vkGetFenceStatus as *const c_void,
        b"vkCreateEvent" => guest_vkCreateEvent as *const c_void,
        b"vkDestroyEvent" => guest_vkDestroyEvent as *const c_void,
        b"vkGetEventStatus" => guest_vkGetEventStatus as *const c_void,
        b"vkSetEvent" => guest_vkSetEvent as *const c_void,
        b"vkResetEvent" => guest_vkResetEvent as *const c_void,
        b"vkGetSemaphoreCounterValue" | b"vkGetSemaphoreCounterValueKHR" => {
            guest_vkGetSemaphoreCounterValue as *const c_void
        }
        b"vkWaitSemaphores" | b"vkWaitSemaphoresKHR" => guest_vkWaitSemaphores as *const c_void,
        b"vkSignalSemaphore" | b"vkSignalSemaphoreKHR" => guest_vkSignalSemaphore as *const c_void,

        // Memory
        b"vkAllocateMemory" => guest_vkAllocateMemory as *const c_void,
        b"vkFreeMemory" => guest_vkFreeMemory as *const c_void,
        b"vkMapMemory" => guest_vkMapMemory as *const c_void,
        b"vkUnmapMemory" => guest_vkUnmapMemory as *const c_void,
        b"vkFlushMappedMemoryRanges" => guest_vkFlushMappedMemoryRanges as *const c_void,
        b"vkInvalidateMappedMemoryRanges" => guest_vkInvalidateMappedMemoryRanges as *const c_void,
        b"vkGetDeviceMemoryCommitment" => guest_vkGetDeviceMemoryCommitment as *const c_void,

        // Buffer / Image
        b"vkCreateBuffer" => guest_vkCreateBuffer as *const c_void,
        b"vkDestroyBuffer" => guest_vkDestroyBuffer as *const c_void,
        b"vkCreateImage" => guest_vkCreateImage as *const c_void,
        b"vkDestroyImage" => guest_vkDestroyImage as *const c_void,
        b"vkCreateImageView" => guest_vkCreateImageView as *const c_void,
        b"vkDestroyImageView" => guest_vkDestroyImageView as *const c_void,
        b"vkCreateBufferView" => guest_vkCreateBufferView as *const c_void,
        b"vkDestroyBufferView" => guest_vkDestroyBufferView as *const c_void,
        b"vkGetBufferMemoryRequirements" => guest_vkGetBufferMemoryRequirements as *const c_void,
        b"vkGetBufferMemoryRequirements2" | b"vkGetBufferMemoryRequirements2KHR" => {
            guest_vkGetBufferMemoryRequirements2 as *const c_void
        }
        b"vkGetImageMemoryRequirements" => guest_vkGetImageMemoryRequirements as *const c_void,
        b"vkGetImageMemoryRequirements2" | b"vkGetImageMemoryRequirements2KHR" => {
            guest_vkGetImageMemoryRequirements2 as *const c_void
        }
        b"vkBindBufferMemory" => guest_vkBindBufferMemory as *const c_void,
        b"vkBindBufferMemory2" | b"vkBindBufferMemory2KHR" => {
            guest_vkBindBufferMemory2 as *const c_void
        }
        b"vkBindImageMemory" => guest_vkBindImageMemory as *const c_void,
        b"vkBindImageMemory2" | b"vkBindImageMemory2KHR" => {
            guest_vkBindImageMemory2 as *const c_void
        }
        b"vkGetImageSubresourceLayout" => guest_vkGetImageSubresourceLayout as *const c_void,

        // Command Pool / Buffer
        b"vkCreateCommandPool" => guest_vkCreateCommandPool as *const c_void,
        b"vkDestroyCommandPool" => guest_vkDestroyCommandPool as *const c_void,
        b"vkResetCommandPool" => guest_vkResetCommandPool as *const c_void,
        b"vkTrimCommandPool" | b"vkTrimCommandPoolKHR" => guest_vkTrimCommandPool as *const c_void,
        b"vkAllocateCommandBuffers" => guest_vkAllocateCommandBuffers as *const c_void,
        b"vkFreeCommandBuffers" => guest_vkFreeCommandBuffers as *const c_void,
        b"vkBeginCommandBuffer" => guest_vkBeginCommandBuffer as *const c_void,
        b"vkEndCommandBuffer" => guest_vkEndCommandBuffer as *const c_void,
        b"vkResetCommandBuffer" => guest_vkResetCommandBuffer as *const c_void,

        // Render Pass / Framebuffer
        b"vkCreateRenderPass" => guest_vkCreateRenderPass as *const c_void,
        b"vkCreateRenderPass2" | b"vkCreateRenderPass2KHR" => {
            guest_vkCreateRenderPass2 as *const c_void
        }
        b"vkDestroyRenderPass" => guest_vkDestroyRenderPass as *const c_void,
        b"vkCreateFramebuffer" => guest_vkCreateFramebuffer as *const c_void,
        b"vkDestroyFramebuffer" => guest_vkDestroyFramebuffer as *const c_void,
        b"vkGetRenderAreaGranularity" => guest_vkGetRenderAreaGranularity as *const c_void,

        // Pipeline
        b"vkCreateShaderModule" => guest_vkCreateShaderModule as *const c_void,
        b"vkDestroyShaderModule" => guest_vkDestroyShaderModule as *const c_void,
        b"vkCreatePipelineCache" => guest_vkCreatePipelineCache as *const c_void,
        b"vkDestroyPipelineCache" => guest_vkDestroyPipelineCache as *const c_void,
        b"vkGetPipelineCacheData" => guest_vkGetPipelineCacheData as *const c_void,
        b"vkMergePipelineCaches" => guest_vkMergePipelineCaches as *const c_void,
        b"vkCreatePipelineLayout" => guest_vkCreatePipelineLayout as *const c_void,
        b"vkDestroyPipelineLayout" => guest_vkDestroyPipelineLayout as *const c_void,
        b"vkCreateGraphicsPipelines" => guest_vkCreateGraphicsPipelines as *const c_void,
        b"vkCreateComputePipelines" => guest_vkCreateComputePipelines as *const c_void,
        b"vkDestroyPipeline" => guest_vkDestroyPipeline as *const c_void,

        // Descriptors
        b"vkCreateDescriptorSetLayout" => guest_vkCreateDescriptorSetLayout as *const c_void,
        b"vkDestroyDescriptorSetLayout" => guest_vkDestroyDescriptorSetLayout as *const c_void,
        b"vkCreateDescriptorPool" => guest_vkCreateDescriptorPool as *const c_void,
        b"vkDestroyDescriptorPool" => guest_vkDestroyDescriptorPool as *const c_void,
        b"vkResetDescriptorPool" => guest_vkResetDescriptorPool as *const c_void,
        b"vkAllocateDescriptorSets" => guest_vkAllocateDescriptorSets as *const c_void,
        b"vkFreeDescriptorSets" => guest_vkFreeDescriptorSets as *const c_void,
        b"vkUpdateDescriptorSets" => guest_vkUpdateDescriptorSets as *const c_void,
        b"vkCreateDescriptorUpdateTemplate" | b"vkCreateDescriptorUpdateTemplateKHR" => {
            guest_vkCreateDescriptorUpdateTemplate as *const c_void
        }
        b"vkDestroyDescriptorUpdateTemplate" | b"vkDestroyDescriptorUpdateTemplateKHR" => {
            guest_vkDestroyDescriptorUpdateTemplate as *const c_void
        }
        b"vkUpdateDescriptorSetWithTemplate" | b"vkUpdateDescriptorSetWithTemplateKHR" => {
            guest_vkUpdateDescriptorSetWithTemplate as *const c_void
        }

        // Sampler / Query
        b"vkCreateSampler" => guest_vkCreateSampler as *const c_void,
        b"vkDestroySampler" => guest_vkDestroySampler as *const c_void,
        b"vkCreateQueryPool" => guest_vkCreateQueryPool as *const c_void,
        b"vkDestroyQueryPool" => guest_vkDestroyQueryPool as *const c_void,
        b"vkGetQueryPoolResults" => guest_vkGetQueryPoolResults as *const c_void,
        b"vkResetQueryPool" | b"vkResetQueryPoolEXT" => guest_vkResetQueryPool as *const c_void,

        // Cmd recording
        b"vkCmdBindPipeline" => guest_vkCmdBindPipeline as *const c_void,
        b"vkCmdSetViewport" => guest_vkCmdSetViewport as *const c_void,
        b"vkCmdSetScissor" => guest_vkCmdSetScissor as *const c_void,
        b"vkCmdSetLineWidth" => guest_vkCmdSetLineWidth as *const c_void,
        b"vkCmdSetDepthBias" => guest_vkCmdSetDepthBias as *const c_void,
        b"vkCmdSetBlendConstants" => guest_vkCmdSetBlendConstants as *const c_void,
        b"vkCmdSetDepthBounds" => guest_vkCmdSetDepthBounds as *const c_void,
        b"vkCmdSetStencilCompareMask" => guest_vkCmdSetStencilCompareMask as *const c_void,
        b"vkCmdSetStencilWriteMask" => guest_vkCmdSetStencilWriteMask as *const c_void,
        b"vkCmdSetStencilReference" => guest_vkCmdSetStencilReference as *const c_void,
        b"vkCmdBindDescriptorSets" => guest_vkCmdBindDescriptorSets as *const c_void,
        b"vkCmdBindIndexBuffer" => guest_vkCmdBindIndexBuffer as *const c_void,
        b"vkCmdBindVertexBuffers" => guest_vkCmdBindVertexBuffers as *const c_void,
        b"vkCmdDraw" => guest_vkCmdDraw as *const c_void,
        b"vkCmdDrawIndexed" => guest_vkCmdDrawIndexed as *const c_void,
        b"vkCmdDrawIndirect" => guest_vkCmdDrawIndirect as *const c_void,
        b"vkCmdDrawIndexedIndirect" => guest_vkCmdDrawIndexedIndirect as *const c_void,
        b"vkCmdDrawIndirectCount" | b"vkCmdDrawIndirectCountKHR" => {
            guest_vkCmdDrawIndirectCount as *const c_void
        }
        b"vkCmdDrawIndexedIndirectCount" | b"vkCmdDrawIndexedIndirectCountKHR" => {
            guest_vkCmdDrawIndexedIndirectCount as *const c_void
        }
        b"vkCmdDispatch" => guest_vkCmdDispatch as *const c_void,
        b"vkCmdDispatchIndirect" => guest_vkCmdDispatchIndirect as *const c_void,
        b"vkCmdCopyBuffer" => guest_vkCmdCopyBuffer as *const c_void,
        b"vkCmdCopyImage" => guest_vkCmdCopyImage as *const c_void,
        b"vkCmdBlitImage" => guest_vkCmdBlitImage as *const c_void,
        b"vkCmdCopyBufferToImage" => guest_vkCmdCopyBufferToImage as *const c_void,
        b"vkCmdCopyImageToBuffer" => guest_vkCmdCopyImageToBuffer as *const c_void,
        b"vkCmdUpdateBuffer" => guest_vkCmdUpdateBuffer as *const c_void,
        b"vkCmdFillBuffer" => guest_vkCmdFillBuffer as *const c_void,
        b"vkCmdClearColorImage" => guest_vkCmdClearColorImage as *const c_void,
        b"vkCmdClearDepthStencilImage" => guest_vkCmdClearDepthStencilImage as *const c_void,
        b"vkCmdClearAttachments" => guest_vkCmdClearAttachments as *const c_void,
        b"vkCmdResolveImage" => guest_vkCmdResolveImage as *const c_void,
        b"vkCmdBeginRenderPass" => guest_vkCmdBeginRenderPass as *const c_void,
        b"vkCmdNextSubpass" => guest_vkCmdNextSubpass as *const c_void,
        b"vkCmdEndRenderPass" => guest_vkCmdEndRenderPass as *const c_void,
        b"vkCmdBeginRenderPass2" | b"vkCmdBeginRenderPass2KHR" => {
            guest_vkCmdBeginRenderPass2 as *const c_void
        }
        b"vkCmdNextSubpass2" | b"vkCmdNextSubpass2KHR" => guest_vkCmdNextSubpass2 as *const c_void,
        b"vkCmdEndRenderPass2" | b"vkCmdEndRenderPass2KHR" => {
            guest_vkCmdEndRenderPass2 as *const c_void
        }
        b"vkCmdPipelineBarrier" => guest_vkCmdPipelineBarrier as *const c_void,
        b"vkCmdPipelineBarrier2" | b"vkCmdPipelineBarrier2KHR" => {
            guest_vkCmdPipelineBarrier2 as *const c_void
        }
        b"vkCmdPushConstants" => guest_vkCmdPushConstants as *const c_void,
        b"vkCmdExecuteCommands" => guest_vkCmdExecuteCommands as *const c_void,
        b"vkCmdBeginQuery" => guest_vkCmdBeginQuery as *const c_void,
        b"vkCmdEndQuery" => guest_vkCmdEndQuery as *const c_void,
        b"vkCmdResetQueryPool" => guest_vkCmdResetQueryPool as *const c_void,
        b"vkCmdWriteTimestamp" => guest_vkCmdWriteTimestamp as *const c_void,
        b"vkCmdCopyQueryPoolResults" => guest_vkCmdCopyQueryPoolResults as *const c_void,
        b"vkCmdSetEvent" => guest_vkCmdSetEvent as *const c_void,
        b"vkCmdResetEvent" => guest_vkCmdResetEvent as *const c_void,
        b"vkCmdWaitEvents" => guest_vkCmdWaitEvents as *const c_void,

        // DXVK Extended Dynamic State
        b"vkCmdSetCullMode" | b"vkCmdSetCullModeEXT" => guest_vkCmdSetCullMode as *const c_void,
        b"vkCmdSetFrontFace" | b"vkCmdSetFrontFaceEXT" => guest_vkCmdSetFrontFace as *const c_void,
        b"vkCmdSetDepthTestEnable" | b"vkCmdSetDepthTestEnableEXT" => {
            guest_vkCmdSetDepthTestEnable as *const c_void
        }
        b"vkCmdSetDepthWriteEnable" | b"vkCmdSetDepthWriteEnableEXT" => {
            guest_vkCmdSetDepthWriteEnable as *const c_void
        }
        b"vkCmdSetDepthCompareOp" | b"vkCmdSetDepthCompareOpEXT" => {
            guest_vkCmdSetDepthCompareOp as *const c_void
        }
        b"vkCmdSetStencilTestEnable" | b"vkCmdSetStencilTestEnableEXT" => {
            guest_vkCmdSetStencilTestEnable as *const c_void
        }
        b"vkCmdSetStencilOp" | b"vkCmdSetStencilOpEXT" => guest_vkCmdSetStencilOp as *const c_void,
        b"vkCmdSetDepthBoundsTestEnable" | b"vkCmdSetDepthBoundsTestEnableEXT" => {
            guest_vkCmdSetDepthBoundsTestEnable as *const c_void
        }
        b"vkCmdSetPrimitiveTopology" | b"vkCmdSetPrimitiveTopologyEXT" => {
            guest_vkCmdSetPrimitiveTopology as *const c_void
        }
        b"vkCmdSetPrimitiveRestartEnable" | b"vkCmdSetPrimitiveRestartEnableEXT" => {
            guest_vkCmdSetPrimitiveRestartEnable as *const c_void
        }
        b"vkCmdSetRasterizerDiscardEnable" | b"vkCmdSetRasterizerDiscardEnableEXT" => {
            guest_vkCmdSetRasterizerDiscardEnable as *const c_void
        }
        b"vkCmdSetDepthBiasEnable" | b"vkCmdSetDepthBiasEnableEXT" => {
            guest_vkCmdSetDepthBiasEnable as *const c_void
        }
        b"vkCmdSetLogicOpEXT" => guest_vkCmdSetLogicOpEXT as *const c_void,
        b"vkCmdSetPatchControlPointsEXT" => guest_vkCmdSetPatchControlPointsEXT as *const c_void,
        b"vkCmdSetColorWriteEnableEXT" => guest_vkCmdSetColorWriteEnableEXT as *const c_void,
        b"vkCmdSetVertexInputEXT" => guest_vkCmdSetVertexInputEXT as *const c_void,

        // Transform Feedback
        b"vkCmdBindTransformFeedbackBuffersEXT" => {
            guest_vkCmdBindTransformFeedbackBuffersEXT as *const c_void
        }
        b"vkCmdBeginTransformFeedbackEXT" => guest_vkCmdBeginTransformFeedbackEXT as *const c_void,
        b"vkCmdEndTransformFeedbackEXT" => guest_vkCmdEndTransformFeedbackEXT as *const c_void,

        // Conditional Rendering
        b"vkCmdBeginConditionalRenderingEXT" => {
            guest_vkCmdBeginConditionalRenderingEXT as *const c_void
        }
        b"vkCmdEndConditionalRenderingEXT" => {
            guest_vkCmdEndConditionalRenderingEXT as *const c_void
        }

        // Debug Utils
        b"vkSetDebugUtilsObjectNameEXT" => guest_vkSetDebugUtilsObjectNameEXT as *const c_void,
        b"vkSetDebugUtilsObjectTagEXT" => guest_vkSetDebugUtilsObjectTagEXT as *const c_void,
        b"vkCmdBeginDebugUtilsLabelEXT" => guest_vkCmdBeginDebugUtilsLabelEXT as *const c_void,
        b"vkCmdEndDebugUtilsLabelEXT" => guest_vkCmdEndDebugUtilsLabelEXT as *const c_void,
        b"vkCmdInsertDebugUtilsLabelEXT" => guest_vkCmdInsertDebugUtilsLabelEXT as *const c_void,
        b"vkQueueBeginDebugUtilsLabelEXT" => guest_vkQueueBeginDebugUtilsLabelEXT as *const c_void,
        b"vkQueueEndDebugUtilsLabelEXT" => guest_vkQueueEndDebugUtilsLabelEXT as *const c_void,
        b"vkQueueInsertDebugUtilsLabelEXT" => {
            guest_vkQueueInsertDebugUtilsLabelEXT as *const c_void
        }

        _ => return None,
    };
    Some(ptr)
}

/// Instance-level dispatch registry: resolves global, instance, physical device, WSI, and device commands.
fn lookup_instance_thunk(name_bytes: &[u8]) -> Option<*const c_void> {
    if let Some(ptr) = lookup_device_thunk(name_bytes) {
        return Some(ptr);
    }
    let ptr: *const c_void = match name_bytes {
        // Global / Loader
        b"vkGetInstanceProcAddr" => rine_vkGetInstanceProcAddr as *const c_void,
        b"vkEnumerateInstanceExtensionProperties" => {
            rine_vkEnumerateInstanceExtensionProperties as *const c_void
        }
        b"vkCreateInstance" => rine_vkCreateInstance as *const c_void,
        b"vkEnumerateInstanceVersion" => guest_vkEnumerateInstanceVersion as *const c_void,
        b"vkEnumerateInstanceLayerProperties" => {
            guest_vkEnumerateInstanceLayerProperties as *const c_void
        }

        // Instance lifecycle
        b"vkDestroyInstance" => guest_vkDestroyInstance as *const c_void,
        b"vkCreateDevice" => guest_vkCreateDevice as *const c_void,

        // Physical device enumeration & properties
        b"vkEnumeratePhysicalDevices" => guest_vkEnumeratePhysicalDevices as *const c_void,
        b"vkGetPhysicalDeviceProperties" => guest_vkGetPhysicalDeviceProperties as *const c_void,
        b"vkGetPhysicalDeviceProperties2" | b"vkGetPhysicalDeviceProperties2KHR" => {
            guest_vkGetPhysicalDeviceProperties2 as *const c_void
        }
        b"vkGetPhysicalDeviceFeatures" => guest_vkGetPhysicalDeviceFeatures as *const c_void,
        b"vkGetPhysicalDeviceFeatures2" | b"vkGetPhysicalDeviceFeatures2KHR" => {
            guest_vkGetPhysicalDeviceFeatures2 as *const c_void
        }
        b"vkGetPhysicalDeviceMemoryProperties" => {
            guest_vkGetPhysicalDeviceMemoryProperties as *const c_void
        }
        b"vkGetPhysicalDeviceMemoryProperties2" | b"vkGetPhysicalDeviceMemoryProperties2KHR" => {
            guest_vkGetPhysicalDeviceMemoryProperties2 as *const c_void
        }
        b"vkGetPhysicalDeviceQueueFamilyProperties" => {
            guest_vkGetPhysicalDeviceQueueFamilyProperties as *const c_void
        }
        b"vkGetPhysicalDeviceQueueFamilyProperties2"
        | b"vkGetPhysicalDeviceQueueFamilyProperties2KHR" => {
            guest_vkGetPhysicalDeviceQueueFamilyProperties2 as *const c_void
        }
        b"vkGetPhysicalDeviceFormatProperties" => {
            guest_vkGetPhysicalDeviceFormatProperties as *const c_void
        }
        b"vkGetPhysicalDeviceFormatProperties2" | b"vkGetPhysicalDeviceFormatProperties2KHR" => {
            guest_vkGetPhysicalDeviceFormatProperties2 as *const c_void
        }
        b"vkGetPhysicalDeviceImageFormatProperties" => {
            guest_vkGetPhysicalDeviceImageFormatProperties as *const c_void
        }
        b"vkGetPhysicalDeviceImageFormatProperties2"
        | b"vkGetPhysicalDeviceImageFormatProperties2KHR" => {
            guest_vkGetPhysicalDeviceImageFormatProperties2 as *const c_void
        }
        b"vkGetPhysicalDeviceSparseImageFormatProperties" => {
            guest_vkGetPhysicalDeviceSparseImageFormatProperties as *const c_void
        }
        b"vkEnumerateDeviceExtensionProperties" => {
            guest_vkEnumerateDeviceExtensionProperties as *const c_void
        }
        b"vkEnumerateDeviceLayerProperties" => {
            guest_vkEnumerateDeviceLayerProperties as *const c_void
        }

        // WSI (Win32 -> Xlib translation)
        b"vkCreateWin32SurfaceKHR" => rine_vkCreateWin32SurfaceKHR as *const c_void,
        b"vkGetPhysicalDeviceWin32PresentationSupportKHR" => {
            rine_vkGetPhysicalDeviceWin32PresentationSupportKHR as *const c_void
        }
        b"vkGetPhysicalDeviceSurfaceSupportKHR" => {
            guest_vkGetPhysicalDeviceSurfaceSupportKHR as *const c_void
        }
        b"vkGetPhysicalDeviceSurfaceCapabilitiesKHR" => {
            guest_vkGetPhysicalDeviceSurfaceCapabilitiesKHR as *const c_void
        }
        b"vkGetPhysicalDeviceSurfaceCapabilities2KHR" => {
            guest_vkGetPhysicalDeviceSurfaceCapabilities2KHR as *const c_void
        }
        b"vkGetPhysicalDeviceSurfaceFormatsKHR" => {
            guest_vkGetPhysicalDeviceSurfaceFormatsKHR as *const c_void
        }
        b"vkGetPhysicalDeviceSurfaceFormats2KHR" => {
            guest_vkGetPhysicalDeviceSurfaceFormats2KHR as *const c_void
        }
        b"vkGetPhysicalDeviceSurfacePresentModesKHR" => {
            guest_vkGetPhysicalDeviceSurfacePresentModesKHR as *const c_void
        }
        b"vkDestroySurfaceKHR" => guest_vkDestroySurfaceKHR as *const c_void,

        _ => return None,
    };
    Some(ptr)
}

pub unsafe extern "win64" fn rine_vkGetInstanceProcAddr(
    _instance: NativeVkInstance,
    p_name: *const c_char,
) -> *const c_void {
    if p_name.is_null() {
        return std::ptr::null();
    }

    let c_str = CStr::from_ptr(p_name);
    let name_bytes = c_str.to_bytes();
    let name_str = c_str.to_string_lossy();

    let thunk = lookup_instance_thunk(name_bytes).unwrap_or(std::ptr::null());
    log_gipa_once(&name_str, thunk);
    thunk
}

pub unsafe extern "win64" fn rine_vkGetDeviceProcAddr(
    _device: NativeVkDevice,
    p_name: *const c_char,
) -> *const c_void {
    if p_name.is_null() {
        return std::ptr::null();
    }

    let c_str = CStr::from_ptr(p_name);
    let name_bytes = c_str.to_bytes();
    let name_str = c_str.to_string_lossy();

    let thunk = lookup_device_thunk(name_bytes).unwrap_or(std::ptr::null());
    log_gdpa_once(&name_str, thunk);
    thunk
}

pub fn resolve_vulkan_symbol(fn_name: &str) -> Option<usize> {
    let name_c = std::ffi::CString::new(fn_name).ok()?;
    let ptr = unsafe { rine_vkGetInstanceProcAddr(std::ptr::null_mut(), name_c.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(ptr as usize)
    }
}
