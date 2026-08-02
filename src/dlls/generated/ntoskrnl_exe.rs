//! Dedicated Rine Rust implementation module for ntoskrnl.exe
#![allow(non_snake_case)]

pub fn CcCanIWrite() -> u64 {
    0
}

pub fn CcCopyRead() -> u64 {
    0
}

pub fn CcCopyWrite() -> u64 {
    0
}

pub fn CcDeferWrite() -> u64 {
    0
}

pub fn CcFastCopyRead() -> u64 {
    0
}

pub fn CcFastCopyWrite() -> u64 {
    0
}

pub fn CcFastMdlReadWait() -> u64 {
    0
}

pub fn CcFastReadNotPossible() -> u64 {
    0
}

pub fn CcFastReadWait() -> u64 {
    0
}

pub fn CcFlushCache() -> u64 {
    0
}

pub fn CcGetDirtyPages() -> u64 {
    0
}

pub fn CcGetFileObjectFromBcb() -> u64 {
    0
}

pub fn CcGetFileObjectFromSectionPtrs() -> u64 {
    0
}

pub fn CcGetFlushedValidData() -> u64 {
    0
}

pub fn CcGetLsnForFileObject() -> u64 {
    0
}

pub fn CcInitializeCacheMap() -> u64 {
    0
}

pub fn CcIsThereDirtyData() -> u64 {
    0
}

pub fn CcMapData() -> u64 {
    0
}

pub fn CcMdlRead() -> u64 {
    0
}

pub fn CcMdlReadComplete() -> u64 {
    0
}

pub fn CcMdlWriteAbort() -> u64 {
    0
}

pub fn CcMdlWriteComplete() -> u64 {
    0
}

pub fn CcPinMappedData() -> u64 {
    0
}

pub fn CcPinRead() -> u64 {
    0
}

pub fn CcPrepareMdlWrite() -> u64 {
    0
}

pub fn CcPreparePinWrite() -> u64 {
    0
}

pub fn CcPurgeCacheSection() -> u64 {
    0
}

pub fn CcRemapBcb() -> u64 {
    0
}

pub fn CcRepinBcb() -> u64 {
    0
}

pub fn CcScheduleReadAhead() -> u64 {
    0
}

pub fn CcSetAdditionalCacheAttributes() -> u64 {
    0
}

pub fn CcSetBcbOwnerPointer() -> u64 {
    0
}

pub fn CcSetDirtyPageThreshold() -> u64 {
    0
}

pub fn CcSetDirtyPinnedData() -> u64 {
    0
}

pub fn CcSetFileSizes() -> u64 {
    0
}

pub fn CcSetLogHandleForFile() -> u64 {
    0
}

pub fn CcSetReadAheadGranularity() -> u64 {
    0
}

pub fn CcUninitializeCacheMap() -> u64 {
    0
}

pub fn CcUnpinData() -> u64 {
    0
}

pub fn CcUnpinDataForThread() -> u64 {
    0
}

pub fn CcUnpinRepinnedBcb() -> u64 {
    0
}

pub fn CcWaitForCurrentLazyWriterActivity() -> u64 {
    0
}

pub fn CcZeroData() -> u64 {
    0
}

pub fn CmRegisterCallback() -> u64 {
    0
}

pub fn CmUnRegisterCallback() -> u64 {
    0
}

pub fn DbgBreakPoint() -> u64 {
    0
}

pub fn DbgBreakPointWithStatus() -> u64 {
    0
}

pub fn DbgLoadImageSymbols() -> u64 {
    0
}

pub fn DbgPrintReturnControlC() -> u64 {
    0
}

pub fn DbgPrompt() -> u64 {
    0
}

pub fn DbgQueryDebugFilterState() -> u64 {
    0
}

pub fn DbgSetDebugFilterState() -> u64 {
    0
}

pub fn EtwRegisterClassicProvider() -> u64 {
    0
}

pub fn EtwUnregister() -> u64 {
    0
}

pub fn ExAcquireFastMutexUnsafe() -> u64 {
    0
}

pub fn ExAcquireResourceExclusiveLite() -> u64 {
    0
}

pub fn ExAcquireResourceSharedLite() -> u64 {
    0
}

pub fn ExAcquireRundownProtection() -> u64 {
    0
}

pub fn ExAcquireRundownProtectionEx() -> u64 {
    0
}

pub fn ExAcquireSharedStarveExclusive() -> u64 {
    0
}

pub fn ExAcquireSharedWaitForExclusive() -> u64 {
    0
}

pub fn ExAllocateFromPagedLookasideList() -> u64 {
    0
}

pub fn ExAllocatePool() -> u64 {
    0
}

pub fn ExAllocatePool2() -> u64 {
    0
}

pub fn ExAllocatePoolWithQuota() -> u64 {
    0
}

pub fn ExAllocatePoolWithQuotaTag() -> u64 {
    0
}

pub fn ExAllocatePoolWithTag() -> u64 {
    0
}

pub fn ExAllocatePoolWithTagPriority() -> u64 {
    0
}

pub fn ExConvertExclusiveToSharedLite() -> u64 {
    0
}

pub fn ExCreateCallback() -> u64 {
    0
}

pub fn ExDeleteNPagedLookasideList() -> u64 {
    0
}

pub fn ExDeletePagedLookasideList() -> u64 {
    0
}

pub fn ExDeleteResourceLite() -> u64 {
    0
}

pub fn ExDesktopObjectType() -> u64 {
    0
}

pub fn ExDisableResourceBoostLite() -> u64 {
    0
}

pub fn ExEnumHandleTable() -> u64 {
    0
}

pub fn ExExtendZone() -> u64 {
    0
}

pub fn ExFreePool() -> u64 {
    0
}

pub fn ExFreePoolWithTag() -> u64 {
    0
}

pub fn ExFreeToPagedLookasideList() -> u64 {
    0
}

pub fn ExGetCurrentProcessorCounts() -> u64 {
    0
}

pub fn ExGetCurrentProcessorCpuUsage() -> u64 {
    0
}

pub fn ExGetExclusiveWaiterCount() -> u64 {
    0
}

pub fn ExGetPreviousMode() -> u64 {
    0
}

pub fn ExGetSharedWaiterCount() -> u64 {
    0
}

pub fn ExInitializeNPagedLookasideList() -> u64 {
    0
}

pub fn ExInitializePagedLookasideList() -> u64 {
    0
}

pub fn ExInitializeResourceLite() -> u64 {
    0
}

pub fn ExInitializeRundownProtection() -> u64 {
    0
}

pub fn ExInitializeZone() -> u64 {
    0
}

pub fn ExInterlockedAddLargeInteger() -> u64 {
    0
}

pub fn ExInterlockedAddLargeStatistic() -> u64 {
    0
}

pub fn ExInterlockedAddUlong() -> u64 {
    0
}

pub fn ExInterlockedCompareExchange64() -> u64 {
    0
}

pub fn ExInterlockedDecrementLong() -> u64 {
    0
}

pub fn ExInterlockedExchangeUlong() -> u64 {
    0
}

pub fn ExInterlockedExtendZone() -> u64 {
    0
}

pub fn ExInterlockedIncrementLong() -> u64 {
    0
}

pub fn ExInterlockedInsertHeadList() -> u64 {
    0
}

pub fn ExInterlockedInsertTailList() -> u64 {
    0
}

pub fn ExInterlockedPopEntryList() -> u64 {
    0
}

pub fn ExInterlockedPushEntryList() -> u64 {
    0
}

pub fn ExInterlockedRemoveHeadList() -> u64 {
    0
}

pub fn ExIsProcessorFeaturePresent() -> u64 {
    0
}

pub fn ExIsResourceAcquiredExclusiveLite() -> u64 {
    0
}

pub fn ExIsResourceAcquiredSharedLite() -> u64 {
    0
}

pub fn ExLocalTimeToSystemTime() -> u64 {
    0
}

pub fn ExNotifyCallback() -> u64 {
    0
}

pub fn ExQueryPoolBlockSize() -> u64 {
    0
}

pub fn ExQueueWorkItem() -> u64 {
    0
}

pub fn ExRaiseAccessViolation() -> u64 {
    0
}

pub fn ExRaiseDatatypeMisalignment() -> u64 {
    0
}

pub fn ExRaiseException() -> u64 {
    0
}

pub fn ExRaiseHardError() -> u64 {
    0
}

pub fn ExRaiseStatus() -> u64 {
    0
}

pub fn ExReInitializeRundownProtection() -> u64 {
    0
}

pub fn ExRegisterCallback() -> u64 {
    0
}

pub fn ExReinitializeResourceLite() -> u64 {
    0
}

pub fn ExReleaseFastMutexUnsafe() -> u64 {
    0
}

pub fn ExReleaseResourceForThreadLite() -> u64 {
    0
}

pub fn ExReleaseResourceLite() -> u64 {
    0
}

pub fn ExReleaseRundownProtection() -> u64 {
    0
}

pub fn ExReleaseRundownProtectionEx() -> u64 {
    0
}

pub fn ExRundownCompleted() -> u64 {
    0
}

pub fn ExSetResourceOwnerPointer() -> u64 {
    0
}

pub fn ExSetTimerResolution() -> u64 {
    0
}

pub fn ExSystemExceptionFilter() -> u64 {
    0
}

pub fn ExSystemTimeToLocalTime() -> u64 {
    0
}

pub fn ExUnregisterCallback() -> u64 {
    0
}

pub fn ExUuidCreate() -> u64 {
    0
}

pub fn ExVerifySuite() -> u64 {
    0
}

pub fn ExWaitForRundownProtectionRelease() -> u64 {
    0
}

pub fn ExWindowStationObjectType() -> u64 {
    0
}

pub fn ExfAcquirePushLockExclusive() -> u64 {
    0
}

pub fn ExfAcquirePushLockShared() -> u64 {
    0
}

pub fn ExfInterlockedAddUlong() -> u64 {
    0
}

pub fn ExfInterlockedCompareExchange64() -> u64 {
    0
}

pub fn ExfInterlockedInsertHeadList() -> u64 {
    0
}

pub fn ExfInterlockedInsertTailList() -> u64 {
    0
}

pub fn ExfInterlockedPopEntryList() -> u64 {
    0
}

pub fn ExfInterlockedPushEntryList() -> u64 {
    0
}

pub fn ExfReleasePushLock() -> u64 {
    0
}

pub fn ExfUnblockPushLock() -> u64 {
    0
}

pub fn Exfi386InterlockedDecrementLong() -> u64 {
    0
}

pub fn Exfi386InterlockedExchangeUlong() -> u64 {
    0
}

pub fn Exfi386InterlockedIncrementLong() -> u64 {
    0
}

pub fn Exi386InterlockedDecrementLong() -> u64 {
    0
}

pub fn Exi386InterlockedExchangeUlong() -> u64 {
    0
}

pub fn Exi386InterlockedIncrementLong() -> u64 {
    0
}

pub fn FsRtlAcquireFileExclusive() -> u64 {
    0
}

pub fn FsRtlAddLargeMcbEntry() -> u64 {
    0
}

pub fn FsRtlAddMcbEntry() -> u64 {
    0
}

pub fn FsRtlAddToTunnelCache() -> u64 {
    0
}

pub fn FsRtlAllocateFileLock() -> u64 {
    0
}

pub fn FsRtlAllocatePool() -> u64 {
    0
}

pub fn FsRtlAllocatePoolWithQuota() -> u64 {
    0
}

pub fn FsRtlAllocatePoolWithQuotaTag() -> u64 {
    0
}

pub fn FsRtlAllocatePoolWithTag() -> u64 {
    0
}

pub fn FsRtlAllocateResource() -> u64 {
    0
}

pub fn FsRtlAreNamesEqual() -> u64 {
    0
}

pub fn FsRtlBalanceReads() -> u64 {
    0
}

pub fn FsRtlCheckLockForReadAccess() -> u64 {
    0
}

pub fn FsRtlCheckLockForWriteAccess() -> u64 {
    0
}

pub fn FsRtlCheckOplock() -> u64 {
    0
}

pub fn FsRtlCopyRead() -> u64 {
    0
}

pub fn FsRtlCopyWrite() -> u64 {
    0
}

pub fn FsRtlCurrentBatchOplock() -> u64 {
    0
}

pub fn FsRtlDeleteKeyFromTunnelCache() -> u64 {
    0
}

pub fn FsRtlDeleteTunnelCache() -> u64 {
    0
}

pub fn FsRtlDeregisterUncProvider() -> u64 {
    0
}

pub fn FsRtlDissectDbcs() -> u64 {
    0
}

pub fn FsRtlDissectName() -> u64 {
    0
}

pub fn FsRtlDoesDbcsContainWildCards() -> u64 {
    0
}

pub fn FsRtlDoesNameContainWildCards() -> u64 {
    0
}

pub fn FsRtlFastCheckLockForRead() -> u64 {
    0
}

pub fn FsRtlFastCheckLockForWrite() -> u64 {
    0
}

pub fn FsRtlFastUnlockAll() -> u64 {
    0
}

pub fn FsRtlFastUnlockAllByKey() -> u64 {
    0
}

pub fn FsRtlFastUnlockSingle() -> u64 {
    0
}

pub fn FsRtlFindInTunnelCache() -> u64 {
    0
}

pub fn FsRtlFreeFileLock() -> u64 {
    0
}

pub fn FsRtlGetFileSize() -> u64 {
    0
}

pub fn FsRtlGetNextFileLock() -> u64 {
    0
}

pub fn FsRtlGetNextLargeMcbEntry() -> u64 {
    0
}

pub fn FsRtlGetNextMcbEntry() -> u64 {
    0
}

pub fn FsRtlIncrementCcFastReadNoWait() -> u64 {
    0
}

pub fn FsRtlIncrementCcFastReadNotPossible() -> u64 {
    0
}

pub fn FsRtlIncrementCcFastReadResourceMiss() -> u64 {
    0
}

pub fn FsRtlIncrementCcFastReadWait() -> u64 {
    0
}

pub fn FsRtlInitializeFileLock() -> u64 {
    0
}

pub fn FsRtlInitializeLargeMcb() -> u64 {
    0
}

pub fn FsRtlInitializeMcb() -> u64 {
    0
}

pub fn FsRtlInitializeOplock() -> u64 {
    0
}

pub fn FsRtlInitializeTunnelCache() -> u64 {
    0
}

pub fn FsRtlInsertPerFileObjectContext() -> u64 {
    0
}

pub fn FsRtlInsertPerStreamContext() -> u64 {
    0
}

pub fn FsRtlIsDbcsInExpression() -> u64 {
    0
}

pub fn FsRtlIsFatDbcsLegal() -> u64 {
    0
}

pub fn FsRtlIsHpfsDbcsLegal() -> u64 {
    0
}

pub fn FsRtlIsNameInExpression() -> u64 {
    0
}

pub fn FsRtlIsNtstatusExpected() -> u64 {
    0
}

pub fn FsRtlIsPagingFile() -> u64 {
    0
}

pub fn FsRtlIsTotalDeviceFailure() -> u64 {
    0
}

pub fn FsRtlLegalAnsiCharacterArray() -> u64 {
    0
}

pub fn FsRtlLookupLargeMcbEntry() -> u64 {
    0
}

pub fn FsRtlLookupLastLargeMcbEntry() -> u64 {
    0
}

pub fn FsRtlLookupLastLargeMcbEntryAndIndex() -> u64 {
    0
}

pub fn FsRtlLookupLastMcbEntry() -> u64 {
    0
}

pub fn FsRtlLookupMcbEntry() -> u64 {
    0
}

pub fn FsRtlLookupPerFileObjectContext() -> u64 {
    0
}

pub fn FsRtlLookupPerStreamContextInternal() -> u64 {
    0
}

pub fn FsRtlMdlRead() -> u64 {
    0
}

pub fn FsRtlMdlReadComplete() -> u64 {
    0
}

pub fn FsRtlMdlReadCompleteDev() -> u64 {
    0
}

pub fn FsRtlMdlReadDev() -> u64 {
    0
}

pub fn FsRtlMdlWriteComplete() -> u64 {
    0
}

pub fn FsRtlMdlWriteCompleteDev() -> u64 {
    0
}

pub fn FsRtlNormalizeNtstatus() -> u64 {
    0
}

pub fn FsRtlNotifyChangeDirectory() -> u64 {
    0
}

pub fn FsRtlNotifyCleanup() -> u64 {
    0
}

pub fn FsRtlNotifyFilterChangeDirectory() -> u64 {
    0
}

pub fn FsRtlNotifyFilterReportChange() -> u64 {
    0
}

pub fn FsRtlNotifyFullChangeDirectory() -> u64 {
    0
}

pub fn FsRtlNotifyFullReportChange() -> u64 {
    0
}

pub fn FsRtlNotifyInitializeSync() -> u64 {
    0
}

pub fn FsRtlNotifyReportChange() -> u64 {
    0
}

pub fn FsRtlNotifyUninitializeSync() -> u64 {
    0
}

pub fn FsRtlNotifyVolumeEvent() -> u64 {
    0
}

pub fn FsRtlNumberOfRunsInLargeMcb() -> u64 {
    0
}

pub fn FsRtlNumberOfRunsInMcb() -> u64 {
    0
}

pub fn FsRtlOplockFsctrl() -> u64 {
    0
}

pub fn FsRtlOplockIsFastIoPossible() -> u64 {
    0
}

pub fn FsRtlPostPagingFileStackOverflow() -> u64 {
    0
}

pub fn FsRtlPostStackOverflow() -> u64 {
    0
}

pub fn FsRtlPrepareMdlWrite() -> u64 {
    0
}

pub fn FsRtlPrepareMdlWriteDev() -> u64 {
    0
}

pub fn FsRtlPrivateLock() -> u64 {
    0
}

pub fn FsRtlProcessFileLock() -> u64 {
    0
}

pub fn FsRtlRegisterFileSystemFilterCallbacks() -> u64 {
    0
}

pub fn FsRtlRegisterUncProvider() -> u64 {
    0
}

pub fn FsRtlReleaseFile() -> u64 {
    0
}

pub fn FsRtlRemoveLargeMcbEntry() -> u64 {
    0
}

pub fn FsRtlRemoveMcbEntry() -> u64 {
    0
}

pub fn FsRtlRemovePerFileObjectContext() -> u64 {
    0
}

pub fn FsRtlRemovePerStreamContext() -> u64 {
    0
}

pub fn FsRtlResetLargeMcb() -> u64 {
    0
}

pub fn FsRtlSplitLargeMcb() -> u64 {
    0
}

pub fn FsRtlSyncVolumes() -> u64 {
    0
}

pub fn FsRtlTeardownPerStreamContexts() -> u64 {
    0
}

pub fn FsRtlTruncateLargeMcb() -> u64 {
    0
}

pub fn FsRtlTruncateMcb() -> u64 {
    0
}

pub fn FsRtlUninitializeFileLock() -> u64 {
    0
}

pub fn FsRtlUninitializeLargeMcb() -> u64 {
    0
}

pub fn FsRtlUninitializeMcb() -> u64 {
    0
}

pub fn FsRtlUninitializeOplock() -> u64 {
    0
}

pub fn HalDispatchTable() -> u64 {
    0
}

pub fn HalExamineMBR() -> u64 {
    0
}

pub fn HalPrivateDispatchTable() -> u64 {
    0
}

pub fn HeadlessDispatch() -> u64 {
    0
}

pub fn InbvAcquireDisplayOwnership() -> u64 {
    0
}

pub fn InbvCheckDisplayOwnership() -> u64 {
    0
}

pub fn InbvDisplayString() -> u64 {
    0
}

pub fn InbvEnableBootDriver() -> u64 {
    0
}

pub fn InbvEnableDisplayString() -> u64 {
    0
}

pub fn InbvInstallDisplayStringFilter() -> u64 {
    0
}

pub fn InbvIsBootDriverInstalled() -> u64 {
    0
}

pub fn InbvNotifyDisplayOwnershipLost() -> u64 {
    0
}

pub fn InbvResetDisplay() -> u64 {
    0
}

pub fn InbvSetScrollRegion() -> u64 {
    0
}

pub fn InbvSetTextColor() -> u64 {
    0
}

pub fn InbvSolidColorFill() -> u64 {
    0
}

pub fn InterlockedCompareExchange() -> u64 {
    0
}

pub fn InterlockedDecrement() -> u64 {
    0
}

pub fn InterlockedExchange() -> u64 {
    0
}

pub fn InterlockedExchangeAdd() -> u64 {
    0
}

pub fn InterlockedIncrement() -> u64 {
    0
}

pub fn IoAcquireCancelSpinLock() -> u64 {
    0
}

pub fn IoAcquireRemoveLockEx() -> u64 {
    0
}

pub fn IoAcquireVpbSpinLock() -> u64 {
    0
}

pub fn IoAdapterObjectType() -> u64 {
    0
}

pub fn IoAllocateAdapterChannel() -> u64 {
    0
}

pub fn IoAllocateController() -> u64 {
    0
}

pub fn IoAllocateDriverObjectExtension() -> u64 {
    0
}

pub fn IoAllocateErrorLogEntry() -> u64 {
    0
}

pub fn IoAllocateIrp() -> u64 {
    0
}

pub fn IoAllocateMdl() -> u64 {
    0
}

pub fn IoAllocateWorkItem() -> u64 {
    0
}

pub fn IoAssignDriveLetters() -> u64 {
    0
}

pub fn IoAssignResources() -> u64 {
    0
}

pub fn IoAttachDevice() -> u64 {
    0
}

pub fn IoAttachDeviceByPointer() -> u64 {
    0
}

pub fn IoAttachDeviceToDeviceStack() -> u64 {
    0
}

pub fn IoAttachDeviceToDeviceStackSafe() -> u64 {
    0
}

pub fn IoBuildAsynchronousFsdRequest() -> u64 {
    0
}

pub fn IoBuildDeviceIoControlRequest() -> u64 {
    0
}

pub fn IoBuildPartialMdl() -> u64 {
    0
}

pub fn IoBuildSynchronousFsdRequest() -> u64 {
    0
}

pub fn IoCallDriver() -> u64 {
    0
}

pub fn IoCancelFileOpen() -> u64 {
    0
}

pub fn IoCancelIrp() -> u64 {
    0
}

pub fn IoCheckDesiredAccess() -> u64 {
    0
}

pub fn IoCheckEaBufferValidity() -> u64 {
    0
}

pub fn IoCheckFunctionAccess() -> u64 {
    0
}

pub fn IoCheckQuerySetFileInformation() -> u64 {
    0
}

pub fn IoCheckQuerySetVolumeInformation() -> u64 {
    0
}

pub fn IoCheckQuotaBufferValidity() -> u64 {
    0
}

pub fn IoCheckShareAccess() -> u64 {
    0
}

pub fn IoCompleteRequest() -> u64 {
    0
}

pub fn IoConnectInterrupt() -> u64 {
    0
}

pub fn IoCreateController() -> u64 {
    0
}

pub fn IoCreateDevice() -> u64 {
    0
}

pub fn IoCreateDeviceSecure() -> u64 {
    0
}

pub fn IoCreateDisk() -> u64 {
    0
}

pub fn IoCreateDriver() -> u64 {
    0
}

pub fn IoCreateFile() -> u64 {
    0
}

pub fn IoCreateFileEx() -> u64 {
    0
}

pub fn IoCreateFileSpecifyDeviceObjectHint() -> u64 {
    0
}

pub fn IoCreateNotificationEvent() -> u64 {
    0
}

pub fn IoCreateStreamFileObject() -> u64 {
    0
}

pub fn IoCreateStreamFileObjectEx() -> u64 {
    0
}

pub fn IoCreateStreamFileObjectLite() -> u64 {
    0
}

pub fn IoCreateSymbolicLink() -> u64 {
    0
}

pub fn IoCreateSynchronizationEvent() -> u64 {
    0
}

pub fn IoCreateUnprotectedSymbolicLink() -> u64 {
    0
}

pub fn IoCsqInitialize() -> u64 {
    0
}

pub fn IoCsqInsertIrp() -> u64 {
    0
}

pub fn IoCsqRemoveIrp() -> u64 {
    0
}

pub fn IoCsqRemoveNextIrp() -> u64 {
    0
}

pub fn IoDeleteController() -> u64 {
    0
}

pub fn IoDeleteDevice() -> u64 {
    0
}

pub fn IoDeleteDriver() -> u64 {
    0
}

pub fn IoDeleteSymbolicLink() -> u64 {
    0
}

pub fn IoDetachDevice() -> u64 {
    0
}

pub fn IoDeviceHandlerObjectSize() -> u64 {
    0
}

pub fn IoDeviceHandlerObjectType() -> u64 {
    0
}

pub fn IoDisconnectInterrupt() -> u64 {
    0
}

pub fn IoEnqueueIrp() -> u64 {
    0
}

pub fn IoEnumerateDeviceObjectList() -> u64 {
    0
}

pub fn IoFastQueryNetworkAttributes() -> u64 {
    0
}

pub fn IoForwardAndCatchIrp() -> u64 {
    0
}

pub fn IoForwardIrpSynchronously() -> u64 {
    0
}

pub fn IoFreeController() -> u64 {
    0
}

pub fn IoFreeErrorLogEntry() -> u64 {
    0
}

pub fn IoFreeIrp() -> u64 {
    0
}

pub fn IoFreeMdl() -> u64 {
    0
}

pub fn IoFreeWorkItem() -> u64 {
    0
}

pub fn IoGetAttachedDevice() -> u64 {
    0
}

pub fn IoGetAttachedDeviceReference() -> u64 {
    0
}

pub fn IoGetBaseFileSystemDeviceObject() -> u64 {
    0
}

pub fn IoGetBootDiskInformation() -> u64 {
    0
}

pub fn IoGetConfigurationInformation() -> u64 {
    0
}

pub fn IoGetCurrentProcess() -> u64 {
    0
}

pub fn IoGetDeviceAttachmentBaseRef() -> u64 {
    0
}

pub fn IoGetDeviceInterfaceAlias() -> u64 {
    0
}

pub fn IoGetDeviceInterfacePropertyData() -> u64 {
    0
}

pub fn IoGetDeviceInterfaces() -> u64 {
    0
}

pub fn IoGetDeviceObjectPointer() -> u64 {
    0
}

pub fn IoGetDeviceProperty() -> u64 {
    0
}

pub fn IoGetDevicePropertyData() -> u64 {
    0
}

pub fn IoGetDeviceToVerify() -> u64 {
    0
}

pub fn IoGetDiskDeviceObject() -> u64 {
    0
}

pub fn IoGetDmaAdapter() -> u64 {
    0
}

pub fn IoGetDriverObjectExtension() -> u64 {
    0
}

pub fn IoGetFileObjectGenericMapping() -> u64 {
    0
}

pub fn IoGetInitialStack() -> u64 {
    0
}

pub fn IoGetLowerDeviceObject() -> u64 {
    0
}

pub fn IoGetRelatedDeviceObject() -> u64 {
    0
}

pub fn IoGetRequestorProcess() -> u64 {
    0
}

pub fn IoGetRequestorProcessId() -> u64 {
    0
}

pub fn IoGetRequestorSessionId() -> u64 {
    0
}

pub fn IoGetStackLimits() -> u64 {
    0
}

pub fn IoGetTopLevelIrp() -> u64 {
    0
}

pub fn IoInitializeIrp() -> u64 {
    0
}

pub fn IoInitializeRemoveLockEx() -> u64 {
    0
}

pub fn IoInitializeTimer() -> u64 {
    0
}

pub fn IoInvalidateDeviceRelations() -> u64 {
    0
}

pub fn IoInvalidateDeviceState() -> u64 {
    0
}

pub fn IoIsFileOriginRemote() -> u64 {
    0
}

pub fn IoIsOperationSynchronous() -> u64 {
    0
}

pub fn IoIsSystemThread() -> u64 {
    0
}

pub fn IoIsValidNameGraftingBuffer() -> u64 {
    0
}

pub fn IoIsWdmVersionAvailable() -> u64 {
    0
}

pub fn IoMakeAssociatedIrp() -> u64 {
    0
}

pub fn IoOpenDeviceInterfaceRegistryKey() -> u64 {
    0
}

pub fn IoOpenDeviceRegistryKey() -> u64 {
    0
}

pub fn IoPageRead() -> u64 {
    0
}

pub fn IoPnPDeliverServicePowerNotification() -> u64 {
    0
}

pub fn IoQueryDeviceDescription() -> u64 {
    0
}

pub fn IoQueryFileDosDeviceName() -> u64 {
    0
}

pub fn IoQueryFileInformation() -> u64 {
    0
}

pub fn IoQueryVolumeInformation() -> u64 {
    0
}

pub fn IoQueueThreadIrp() -> u64 {
    0
}

pub fn IoQueueWorkItem() -> u64 {
    0
}

pub fn IoRaiseHardError() -> u64 {
    0
}

pub fn IoRaiseInformationalHardError() -> u64 {
    0
}

pub fn IoReadDiskSignature() -> u64 {
    0
}

pub fn IoReadOperationCount() -> u64 {
    0
}

pub fn IoReadPartitionTable() -> u64 {
    0
}

pub fn IoReadPartitionTableEx() -> u64 {
    0
}

pub fn IoReadTransferCount() -> u64 {
    0
}

pub fn IoRegisterBootDriverReinitialization() -> u64 {
    0
}

pub fn IoRegisterDeviceInterface() -> u64 {
    0
}

pub fn IoRegisterDriverReinitialization() -> u64 {
    0
}

pub fn IoRegisterFileSystem() -> u64 {
    0
}

pub fn IoRegisterFsRegistrationChange() -> u64 {
    0
}

pub fn IoRegisterLastChanceShutdownNotification() -> u64 {
    0
}

pub fn IoRegisterPlugPlayNotification() -> u64 {
    0
}

pub fn IoRegisterShutdownNotification() -> u64 {
    0
}

pub fn IoReleaseCancelSpinLock() -> u64 {
    0
}

pub fn IoReleaseRemoveLockAndWaitEx() -> u64 {
    0
}

pub fn IoReleaseRemoveLockEx() -> u64 {
    0
}

pub fn IoReleaseVpbSpinLock() -> u64 {
    0
}

pub fn IoRemoveShareAccess() -> u64 {
    0
}

pub fn IoReportDetectedDevice() -> u64 {
    0
}

pub fn IoReportHalResourceUsage() -> u64 {
    0
}

pub fn IoReportResourceForDetection() -> u64 {
    0
}

pub fn IoReportResourceUsage() -> u64 {
    0
}

pub fn IoReportTargetDeviceChange() -> u64 {
    0
}

pub fn IoReportTargetDeviceChangeAsynchronous() -> u64 {
    0
}

pub fn IoRequestDeviceEject() -> u64 {
    0
}

pub fn IoReuseIrp() -> u64 {
    0
}

pub fn IoSetCompletionRoutineEx() -> u64 {
    0
}

pub fn IoSetDeviceInterfacePropertyData() -> u64 {
    0
}

pub fn IoSetDeviceInterfaceState() -> u64 {
    0
}

pub fn IoSetDevicePropertyData() -> u64 {
    0
}

pub fn IoSetDeviceToVerify() -> u64 {
    0
}

pub fn IoSetFileOrigin() -> u64 {
    0
}

pub fn IoSetHardErrorOrVerifyDevice() -> u64 {
    0
}

pub fn IoSetInformation() -> u64 {
    0
}

pub fn IoSetIoCompletion() -> u64 {
    0
}

pub fn IoSetPartitionInformation() -> u64 {
    0
}

pub fn IoSetPartitionInformationEx() -> u64 {
    0
}

pub fn IoSetShareAccess() -> u64 {
    0
}

pub fn IoSetStartIoAttributes() -> u64 {
    0
}

pub fn IoSetSystemPartition() -> u64 {
    0
}

pub fn IoSetThreadHardErrorMode() -> u64 {
    0
}

pub fn IoSetTopLevelIrp() -> u64 {
    0
}

pub fn IoStartNextPacket() -> u64 {
    0
}

pub fn IoStartNextPacketByKey() -> u64 {
    0
}

pub fn IoStartPacket() -> u64 {
    0
}

pub fn IoStartTimer() -> u64 {
    0
}

pub fn IoStatisticsLock() -> u64 {
    0
}

pub fn IoStopTimer() -> u64 {
    0
}

pub fn IoSynchronousInvalidateDeviceRelations() -> u64 {
    0
}

pub fn IoSynchronousPageWrite() -> u64 {
    0
}

pub fn IoThreadToProcess() -> u64 {
    0
}

pub fn IoUnregisterFileSystem() -> u64 {
    0
}

pub fn IoUnregisterFsRegistrationChange() -> u64 {
    0
}

pub fn IoUnregisterPlugPlayNotification() -> u64 {
    0
}

pub fn IoUnregisterShutdownNotification() -> u64 {
    0
}

pub fn IoUpdateShareAccess() -> u64 {
    0
}

pub fn IoValidateDeviceIoControlAccess() -> u64 {
    0
}

pub fn IoVerifyPartitionTable() -> u64 {
    0
}

pub fn IoVerifyVolume() -> u64 {
    0
}

pub fn IoVolumeDeviceToDosName() -> u64 {
    0
}

pub fn IoWMIAllocateInstanceIds() -> u64 {
    0
}

pub fn IoWMIDeviceObjectToInstanceName() -> u64 {
    0
}

pub fn IoWMIExecuteMethod() -> u64 {
    0
}

pub fn IoWMIHandleToInstanceName() -> u64 {
    0
}

pub fn IoWMIOpenBlock() -> u64 {
    0
}

pub fn IoWMIQueryAllData() -> u64 {
    0
}

pub fn IoWMIQueryAllDataMultiple() -> u64 {
    0
}

pub fn IoWMIQuerySingleInstance() -> u64 {
    0
}

pub fn IoWMIQuerySingleInstanceMultiple() -> u64 {
    0
}

pub fn IoWMIRegistrationControl() -> u64 {
    0
}

pub fn IoWMISetNotificationCallback() -> u64 {
    0
}

pub fn IoWMISetSingleInstance() -> u64 {
    0
}

pub fn IoWMISetSingleItem() -> u64 {
    0
}

pub fn IoWMISuggestInstanceName() -> u64 {
    0
}

pub fn IoWMIWriteEvent() -> u64 {
    0
}

pub fn IoWriteErrorLogEntry() -> u64 {
    0
}

pub fn IoWriteOperationCount() -> u64 {
    0
}

pub fn IoWritePartitionTable() -> u64 {
    0
}

pub fn IoWritePartitionTableEx() -> u64 {
    0
}

pub fn IoWriteTransferCount() -> u64 {
    0
}

pub fn IofCallDriver() -> u64 {
    0
}

pub fn IofCompleteRequest() -> u64 {
    0
}

pub fn KdChangeOption() -> u64 {
    0
}

pub fn KdDebuggerNotPresent() -> u64 {
    0
}

pub fn KdDisableDebugger() -> u64 {
    0
}

pub fn KdEnableDebugger() -> u64 {
    0
}

pub fn KdEnteredDebugger() -> u64 {
    0
}

pub fn KdPollBreakIn() -> u64 {
    0
}

pub fn KdPowerTransition() -> u64 {
    0
}

pub fn KdRefreshDebuggerNotPresent() -> u64 {
    0
}

pub fn Ke386CallBios() -> u64 {
    0
}

pub fn Ke386IoSetAccessProcess() -> u64 {
    0
}

pub fn Ke386QueryIoAccessMap() -> u64 {
    0
}

pub fn Ke386SetIoAccessMap() -> u64 {
    0
}

pub fn KeAcquireGuardedMutex() -> u64 {
    0
}

pub fn KeAcquireGuardedMutexUnsafe() -> u64 {
    0
}

pub fn KeAcquireInStackQueuedSpinLockAtDpcLevel() -> u64 {
    0
}

pub fn KeAcquireInterruptSpinLock() -> u64 {
    0
}

pub fn KeAcquireSpinLockAtDpcLevel() -> u64 {
    0
}

pub fn KeAddSystemServiceTable() -> u64 {
    0
}

pub fn KeAlertThread() -> u64 {
    0
}

pub fn KeAreAllApcsDisabled() -> u64 {
    0
}

pub fn KeAreApcsDisabled() -> u64 {
    0
}

pub fn KeAttachProcess() -> u64 {
    0
}

pub fn KeBugCheck() -> u64 {
    0
}

pub fn KeBugCheckEx() -> u64 {
    0
}

pub fn KeCancelTimer() -> u64 {
    0
}

pub fn KeCapturePersistentThreadState() -> u64 {
    0
}

pub fn KeClearEvent() -> u64 {
    0
}

pub fn KeConnectInterrupt() -> u64 {
    0
}

pub fn KeDcacheFlushCount() -> u64 {
    0
}

pub fn KeDelayExecutionThread() -> u64 {
    0
}

pub fn KeDeregisterBugCheckCallback() -> u64 {
    0
}

pub fn KeDeregisterBugCheckReasonCallback() -> u64 {
    0
}

pub fn KeDetachProcess() -> u64 {
    0
}

pub fn KeDisconnectInterrupt() -> u64 {
    0
}

pub fn KeEnterCriticalRegion() -> u64 {
    0
}

pub fn KeEnterGuardedRegion() -> u64 {
    0
}

pub fn KeEnterKernelDebugger() -> u64 {
    0
}

pub fn KeExpandKernelStackAndCallout() -> u64 {
    0
}

pub fn KeExpandKernelStackAndCalloutEx() -> u64 {
    0
}

pub fn KeFindConfigurationEntry() -> u64 {
    0
}

pub fn KeFindConfigurationNextEntry() -> u64 {
    0
}

pub fn KeFlushEntireTb() -> u64 {
    0
}

pub fn KeFlushQueuedDpcs() -> u64 {
    0
}

pub fn KeGenericCallDpc() -> u64 {
    0
}

pub fn KeGetCurrentProcessorNumber() -> u64 {
    0
}

pub fn KeGetCurrentProcessorNumberEx() -> u64 {
    0
}

pub fn KeGetCurrentThread() -> u64 {
    0
}

pub fn KeGetPreviousMode() -> u64 {
    0
}

pub fn KeGetRecommendedSharedDataAlignment() -> u64 {
    0
}

pub fn KeI386AbiosCall() -> u64 {
    0
}

pub fn KeI386AllocateGdtSelectors() -> u64 {
    0
}

pub fn KeI386Call16BitCStyleFunction() -> u64 {
    0
}

pub fn KeI386Call16BitFunction() -> u64 {
    0
}

pub fn KeI386FlatToGdtSelector() -> u64 {
    0
}

pub fn KeI386GetLid() -> u64 {
    0
}

pub fn KeI386MachineType() -> u64 {
    0
}

pub fn KeI386ReleaseGdtSelectors() -> u64 {
    0
}

pub fn KeI386ReleaseLid() -> u64 {
    0
}

pub fn KeI386SetGdtSelector() -> u64 {
    0
}

pub fn KeIcacheFlushCount() -> u64 {
    0
}

pub fn KeInitializeApc() -> u64 {
    0
}

pub fn KeInitializeDeviceQueue() -> u64 {
    0
}

pub fn KeInitializeDpc() -> u64 {
    0
}

pub fn KeInitializeEvent() -> u64 {
    0
}

pub fn KeInitializeGuardedMutex() -> u64 {
    0
}

pub fn KeInitializeInterrupt() -> u64 {
    0
}

pub fn KeInitializeMutant() -> u64 {
    0
}

pub fn KeInitializeMutex() -> u64 {
    0
}

pub fn KeInitializeQueue() -> u64 {
    0
}

pub fn KeInitializeSemaphore() -> u64 {
    0
}

pub fn KeInitializeSpinLock() -> u64 {
    0
}

pub fn KeInitializeTimer() -> u64 {
    0
}

pub fn KeInitializeTimerEx() -> u64 {
    0
}

pub fn KeInsertByKeyDeviceQueue() -> u64 {
    0
}

pub fn KeInsertDeviceQueue() -> u64 {
    0
}

pub fn KeInsertHeadQueue() -> u64 {
    0
}

pub fn KeInsertQueue() -> u64 {
    0
}

pub fn KeInsertQueueApc() -> u64 {
    0
}

pub fn KeInsertQueueDpc() -> u64 {
    0
}

pub fn KeIsAttachedProcess() -> u64 {
    0
}

pub fn KeIsExecutingDpc() -> u64 {
    0
}

pub fn KeLeaveCriticalRegion() -> u64 {
    0
}

pub fn KeLeaveGuardedRegion() -> u64 {
    0
}

pub fn KeLoaderBlock() -> u64 {
    0
}

pub fn KeNumberProcessors() -> u64 {
    0
}

pub fn KeProfileInterrupt() -> u64 {
    0
}

pub fn KeProfileInterruptWithSource() -> u64 {
    0
}

pub fn KePulseEvent() -> u64 {
    0
}

pub fn KeQueryActiveGroupCount() -> u64 {
    0
}

pub fn KeQueryActiveProcessorCount() -> u64 {
    0
}

pub fn KeQueryActiveProcessorCountEx() -> u64 {
    0
}

pub fn KeQueryActiveProcessors() -> u64 {
    0
}

pub fn KeQueryGroupAffinity() -> u64 {
    0
}

pub fn KeQueryInterruptTime() -> u64 {
    0
}

pub fn KeQueryMaximumGroupCount() -> u64 {
    0
}

pub fn KeQueryMaximumProcessorCount() -> u64 {
    0
}

pub fn KeQueryMaximumProcessorCountEx() -> u64 {
    0
}

pub fn KeQueryPriorityThread() -> u64 {
    0
}

pub fn KeQueryRuntimeThread() -> u64 {
    0
}

pub fn KeQuerySystemTime() -> u64 {
    0
}

pub fn KeQueryTickCount() -> u64 {
    0
}

pub fn KeQueryTimeIncrement() -> u64 {
    0
}

pub fn KeRaiseUserException() -> u64 {
    0
}

pub fn KeReadStateEvent() -> u64 {
    0
}

pub fn KeReadStateMutant() -> u64 {
    0
}

pub fn KeReadStateMutex() -> u64 {
    0
}

pub fn KeReadStateQueue() -> u64 {
    0
}

pub fn KeReadStateSemaphore() -> u64 {
    0
}

pub fn KeReadStateTimer() -> u64 {
    0
}

pub fn KeRegisterBugCheckCallback() -> u64 {
    0
}

pub fn KeRegisterBugCheckReasonCallback() -> u64 {
    0
}

pub fn KeReleaseGuardedMutex() -> u64 {
    0
}

pub fn KeReleaseGuardedMutexUnsafe() -> u64 {
    0
}

pub fn KeReleaseInStackQueuedSpinLockFromDpcLevel() -> u64 {
    0
}

pub fn KeReleaseInterruptSpinLock() -> u64 {
    0
}

pub fn KeReleaseMutant() -> u64 {
    0
}

pub fn KeReleaseMutex() -> u64 {
    0
}

pub fn KeReleaseSemaphore() -> u64 {
    0
}

pub fn KeReleaseSpinLockFromDpcLevel() -> u64 {
    0
}

pub fn KeRemoveByKeyDeviceQueue() -> u64 {
    0
}

pub fn KeRemoveByKeyDeviceQueueIfBusy() -> u64 {
    0
}

pub fn KeRemoveDeviceQueue() -> u64 {
    0
}

pub fn KeRemoveEntryDeviceQueue() -> u64 {
    0
}

pub fn KeRemoveQueue() -> u64 {
    0
}

pub fn KeRemoveQueueDpc() -> u64 {
    0
}

pub fn KeRemoveSystemServiceTable() -> u64 {
    0
}

pub fn KeResetEvent() -> u64 {
    0
}

pub fn KeRestoreFloatingPointState() -> u64 {
    0
}

pub fn KeRevertToUserAffinityThread() -> u64 {
    0
}

pub fn KeRevertToUserAffinityThreadEx() -> u64 {
    0
}

pub fn KeRundownQueue() -> u64 {
    0
}

pub fn KeSaveFloatingPointState() -> u64 {
    0
}

pub fn KeSaveStateForHibernate() -> u64 {
    0
}

pub fn KeSetAffinityThread() -> u64 {
    0
}

pub fn KeSetBasePriorityThread() -> u64 {
    0
}

pub fn KeSetDmaIoCoherency() -> u64 {
    0
}

pub fn KeSetEvent() -> u64 {
    0
}

pub fn KeSetEventBoostPriority() -> u64 {
    0
}

pub fn KeSetIdealProcessorThread() -> u64 {
    0
}

pub fn KeSetImportanceDpc() -> u64 {
    0
}

pub fn KeSetKernelStackSwapEnable() -> u64 {
    0
}

pub fn KeSetPriorityThread() -> u64 {
    0
}

pub fn KeSetProfileIrql() -> u64 {
    0
}

pub fn KeSetSystemAffinityThread() -> u64 {
    0
}

pub fn KeSetSystemAffinityThreadEx() -> u64 {
    0
}

pub fn KeSetTargetProcessorDpc() -> u64 {
    0
}

pub fn KeSetTargetProcessorDpcEx() -> u64 {
    0
}

pub fn KeSetTimeIncrement() -> u64 {
    0
}

pub fn KeSetTimeUpdateNotifyRoutine() -> u64 {
    0
}

pub fn KeSetTimer() -> u64 {
    0
}

pub fn KeSetTimerEx() -> u64 {
    0
}

pub fn KeSignalCallDpcDone() -> u64 {
    0
}

pub fn KeSignalCallDpcSynchronize() -> u64 {
    0
}

pub fn KeStackAttachProcess() -> u64 {
    0
}

pub fn KeSynchronizeExecution() -> u64 {
    0
}

pub fn KeTerminateThread() -> u64 {
    0
}

pub fn KeTestAlertThread() -> u64 {
    0
}

pub fn KeUnstackDetachProcess() -> u64 {
    0
}

pub fn KeUpdateRunTime() -> u64 {
    0
}

pub fn KeUpdateSystemTime() -> u64 {
    0
}

pub fn KeUserModeCallback() -> u64 {
    0
}

pub fn KeWaitForMultipleObjects() -> u64 {
    0
}

pub fn KeWaitForMutexObject() -> u64 {
    0
}

pub fn KeWaitForSingleObject() -> u64 {
    0
}

pub fn KefAcquireSpinLockAtDpcLevel() -> u64 {
    0
}

pub fn KefReleaseSpinLockFromDpcLevel() -> u64 {
    0
}

pub fn Kei386EoiHelper() -> u64 {
    0
}

pub fn KiAcquireSpinLock() -> u64 {
    0
}

pub fn KiBugCheckData() -> u64 {
    0
}

pub fn KiCoprocessorError() -> u64 {
    0
}

pub fn KiDeliverApc() -> u64 {
    0
}

pub fn KiDispatchInterrupt() -> u64 {
    0
}

pub fn KiEnableTimerWatchdog() -> u64 {
    0
}

pub fn KiIpiServiceRoutine() -> u64 {
    0
}

pub fn KiReleaseSpinLock() -> u64 {
    0
}

pub fn KiUnexpectedInterrupt() -> u64 {
    0
}

pub fn Kii386SpinOnSpinLock() -> u64 {
    0
}

pub fn LdrAccessResource() -> u64 {
    0
}

pub fn LdrEnumResources() -> u64 {
    0
}

pub fn LdrFindResourceDirectory_U() -> u64 {
    0
}

pub fn LdrFindResource_U() -> u64 {
    0
}

pub fn LpcPortObjectType() -> u64 {
    0
}

pub fn LpcRequestPort() -> u64 {
    0
}

pub fn LpcRequestWaitReplyPort() -> u64 {
    0
}

pub fn LsaCallAuthenticationPackage() -> u64 {
    0
}

pub fn LsaDeregisterLogonProcess() -> u64 {
    0
}

pub fn LsaFreeReturnBuffer() -> u64 {
    0
}

pub fn LsaLogonUser() -> u64 {
    0
}

pub fn LsaLookupAuthenticationPackage() -> u64 {
    0
}

pub fn LsaRegisterLogonProcess() -> u64 {
    0
}

pub fn Mm64BitPhysicalAddress() -> u64 {
    0
}

pub fn MmAddPhysicalMemory() -> u64 {
    0
}

pub fn MmAddVerifierThunks() -> u64 {
    0
}

pub fn MmAdjustWorkingSetSize() -> u64 {
    0
}

pub fn MmAdvanceMdl() -> u64 {
    0
}

pub fn MmAllocateContiguousMemory() -> u64 {
    0
}

pub fn MmAllocateContiguousMemorySpecifyCache() -> u64 {
    0
}

pub fn MmAllocateMappingAddress() -> u64 {
    0
}

pub fn MmAllocateNonCachedMemory() -> u64 {
    0
}

pub fn MmAllocatePagesForMdl() -> u64 {
    0
}

pub fn MmBuildMdlForNonPagedPool() -> u64 {
    0
}

pub fn MmCanFileBeTruncated() -> u64 {
    0
}

pub fn MmCommitSessionMappedView() -> u64 {
    0
}

pub fn MmCopyVirtualMemory() -> u64 {
    0
}

pub fn MmCreateMdl() -> u64 {
    0
}

pub fn MmCreateSection() -> u64 {
    0
}

pub fn MmDisableModifiedWriteOfSection() -> u64 {
    0
}

pub fn MmFlushImageSection() -> u64 {
    0
}

pub fn MmForceSectionClosed() -> u64 {
    0
}

pub fn MmFreeContiguousMemory() -> u64 {
    0
}

pub fn MmFreeContiguousMemorySpecifyCache() -> u64 {
    0
}

pub fn MmFreeMappingAddress() -> u64 {
    0
}

pub fn MmFreeNonCachedMemory() -> u64 {
    0
}

pub fn MmFreePagesFromMdl() -> u64 {
    0
}

pub fn MmGetPhysicalAddress() -> u64 {
    0
}

pub fn MmGetPhysicalMemoryRanges() -> u64 {
    0
}

pub fn MmGetSystemRoutineAddress() -> u64 {
    0
}

pub fn MmGetVirtualForPhysical() -> u64 {
    0
}

pub fn MmGrowKernelStack() -> u64 {
    0
}

pub fn MmHighestUserAddress() -> u64 {
    0
}

pub fn MmIsAddressValid() -> u64 {
    0
}

pub fn MmIsDriverVerifying() -> u64 {
    0
}

pub fn MmIsNonPagedSystemAddressValid() -> u64 {
    0
}

pub fn MmIsRecursiveIoFault() -> u64 {
    0
}

pub fn MmIsThisAnNtAsSystem() -> u64 {
    0
}

pub fn MmIsVerifierEnabled() -> u64 {
    0
}

pub fn MmLockPagableDataSection() -> u64 {
    0
}

pub fn MmLockPagableImageSection() -> u64 {
    0
}

pub fn MmLockPagableSectionByHandle() -> u64 {
    0
}

pub fn MmMapIoSpace() -> u64 {
    0
}

pub fn MmMapLockedPages() -> u64 {
    0
}

pub fn MmMapLockedPagesSpecifyCache() -> u64 {
    0
}

pub fn MmMapLockedPagesWithReservedMapping() -> u64 {
    0
}

pub fn MmMapMemoryDumpMdl() -> u64 {
    0
}

pub fn MmMapUserAddressesToPage() -> u64 {
    0
}

pub fn MmMapVideoDisplay() -> u64 {
    0
}

pub fn MmMapViewInSessionSpace() -> u64 {
    0
}

pub fn MmMapViewInSystemSpace() -> u64 {
    0
}

pub fn MmMapViewOfSection() -> u64 {
    0
}

pub fn MmMarkPhysicalMemoryAsBad() -> u64 {
    0
}

pub fn MmMarkPhysicalMemoryAsGood() -> u64 {
    0
}

pub fn MmPageEntireDriver() -> u64 {
    0
}

pub fn MmPrefetchPages() -> u64 {
    0
}

pub fn MmProbeAndLockPages() -> u64 {
    0
}

pub fn MmProbeAndLockProcessPages() -> u64 {
    0
}

pub fn MmProbeAndLockSelectedPages() -> u64 {
    0
}

pub fn MmProtectMdlSystemAddress() -> u64 {
    0
}

pub fn MmQuerySystemSize() -> u64 {
    0
}

pub fn MmRemovePhysicalMemory() -> u64 {
    0
}

pub fn MmResetDriverPaging() -> u64 {
    0
}

pub fn MmSectionObjectType() -> u64 {
    0
}

pub fn MmSecureVirtualMemory() -> u64 {
    0
}

pub fn MmSetAddressRangeModified() -> u64 {
    0
}

pub fn MmSetBankedSection() -> u64 {
    0
}

pub fn MmSizeOfMdl() -> u64 {
    0
}

pub fn MmSystemRangeStart() -> u64 {
    0
}

pub fn MmTrimAllSystemPagableMemory() -> u64 {
    0
}

pub fn MmUnlockPagableImageSection() -> u64 {
    0
}

pub fn MmUnlockPages() -> u64 {
    0
}

pub fn MmUnmapIoSpace() -> u64 {
    0
}

pub fn MmUnmapLockedPages() -> u64 {
    0
}

pub fn MmUnmapReservedMapping() -> u64 {
    0
}

pub fn MmUnmapVideoDisplay() -> u64 {
    0
}

pub fn MmUnmapViewInSessionSpace() -> u64 {
    0
}

pub fn MmUnmapViewInSystemSpace() -> u64 {
    0
}

pub fn MmUnmapViewOfSection() -> u64 {
    0
}

pub fn MmUnsecureVirtualMemory() -> u64 {
    0
}

pub fn MmUserProbeAddress() -> u64 {
    0
}

pub fn NlsLeadByteInfo() -> u64 {
    0
}

pub fn NlsOemCodePage() -> u64 {
    0
}

pub fn NlsOemLeadByteInfo() -> u64 {
    0
}

pub fn NtAddAtom() -> u64 {
    0
}

pub fn NtAdjustPrivilegesToken() -> u64 {
    0
}

pub fn NtAllocateLocallyUniqueId() -> u64 {
    0
}

pub fn NtAllocateUuids() -> u64 {
    0
}

pub fn NtAllocateVirtualMemory() -> u64 {
    0
}

pub fn NtClose() -> u64 {
    0
}

pub fn NtConnectPort() -> u64 {
    0
}

pub fn NtCreateEvent() -> u64 {
    0
}

pub fn NtCreateFile() -> u64 {
    0
}

pub fn NtCreateSection() -> u64 {
    0
}

pub fn NtDeleteAtom() -> u64 {
    0
}

pub fn NtDeleteFile() -> u64 {
    0
}

pub fn NtDeviceIoControlFile() -> u64 {
    0
}

pub fn NtDuplicateObject() -> u64 {
    0
}

pub fn NtDuplicateToken() -> u64 {
    0
}

pub fn NtFindAtom() -> u64 {
    0
}

pub fn NtFreeVirtualMemory() -> u64 {
    0
}

pub fn NtFsControlFile() -> u64 {
    0
}

pub fn NtGlobalFlag() -> u64 {
    0
}

pub fn NtLockFile() -> u64 {
    0
}

pub fn NtMapViewOfSection() -> u64 {
    0
}

pub fn NtNotifyChangeDirectoryFile() -> u64 {
    0
}

pub fn NtOpenFile() -> u64 {
    0
}

pub fn NtOpenProcess() -> u64 {
    0
}

pub fn NtOpenProcessToken() -> u64 {
    0
}

pub fn NtOpenProcessTokenEx() -> u64 {
    0
}

pub fn NtOpenThread() -> u64 {
    0
}

pub fn NtOpenThreadToken() -> u64 {
    0
}

pub fn NtOpenThreadTokenEx() -> u64 {
    0
}

pub fn NtQueryDirectoryFile() -> u64 {
    0
}

pub fn NtQueryEaFile() -> u64 {
    0
}

pub fn NtQueryInformationAtom() -> u64 {
    0
}

pub fn NtQueryInformationFile() -> u64 {
    0
}

pub fn NtQueryInformationProcess() -> u64 {
    0
}

pub fn NtQueryInformationThread() -> u64 {
    0
}

pub fn NtQueryInformationToken() -> u64 {
    0
}

pub fn NtQueryQuotaInformationFile() -> u64 {
    0
}

pub fn NtQuerySecurityObject() -> u64 {
    0
}

pub fn NtQuerySystemInformation() -> u64 {
    0
}

pub fn NtQuerySystemInformationEx() -> u64 {
    0
}

pub fn NtQueryVolumeInformationFile() -> u64 {
    0
}

pub fn NtReadFile() -> u64 {
    0
}

pub fn NtRequestPort() -> u64 {
    0
}

pub fn NtRequestWaitReplyPort() -> u64 {
    0
}

pub fn NtSetEaFile() -> u64 {
    0
}

pub fn NtSetEvent() -> u64 {
    0
}

pub fn NtSetEventBoostPriority() -> u64 {
    0
}

pub fn NtSetInformationFile() -> u64 {
    0
}

pub fn NtSetInformationProcess() -> u64 {
    0
}

pub fn NtSetInformationThread() -> u64 {
    0
}

pub fn NtSetInformationToken() -> u64 {
    0
}

pub fn NtSetQuotaInformationFile() -> u64 {
    0
}

pub fn NtSetSecurityObject() -> u64 {
    0
}

pub fn NtSetVolumeInformationFile() -> u64 {
    0
}

pub fn NtShutdownSystem() -> u64 {
    0
}

pub fn NtTraceControl() -> u64 {
    0
}

pub fn NtTraceEvent() -> u64 {
    0
}

pub fn NtUnlockFile() -> u64 {
    0
}

pub fn NtVdmControl() -> u64 {
    0
}

pub fn NtWaitForSingleObject() -> u64 {
    0
}

pub fn NtWriteFile() -> u64 {
    0
}

pub fn ObAssignSecurity() -> u64 {
    0
}

pub fn ObCheckCreateObjectAccess() -> u64 {
    0
}

pub fn ObCheckObjectAccess() -> u64 {
    0
}

pub fn ObCloseHandle() -> u64 {
    0
}

pub fn ObCreateObject() -> u64 {
    0
}

pub fn ObCreateObjectType() -> u64 {
    0
}

pub fn ObDereferenceObject() -> u64 {
    0
}

pub fn ObDereferenceSecurityDescriptor() -> u64 {
    0
}

pub fn ObFindHandleForObject() -> u64 {
    0
}

pub fn ObGetFilterVersion() -> u64 {
    0
}

pub fn ObGetObjectSecurity() -> u64 {
    0
}

pub fn ObGetObjectType() -> u64 {
    0
}

pub fn ObInsertObject() -> u64 {
    0
}

pub fn ObLogSecurityDescriptor() -> u64 {
    0
}

pub fn ObMakeTemporaryObject() -> u64 {
    0
}

pub fn ObOpenObjectByName() -> u64 {
    0
}

pub fn ObOpenObjectByPointer() -> u64 {
    0
}

pub fn ObQueryNameString() -> u64 {
    0
}

pub fn ObQueryObjectAuditingByHandle() -> u64 {
    0
}

pub fn ObReferenceObjectByHandle() -> u64 {
    0
}

pub fn ObReferenceObjectByName() -> u64 {
    0
}

pub fn ObReferenceObjectByPointer() -> u64 {
    0
}

pub fn ObReferenceSecurityDescriptor() -> u64 {
    0
}

pub fn ObRegisterCallbacks() -> u64 {
    0
}

pub fn ObReleaseObjectSecurity() -> u64 {
    0
}

pub fn ObSetHandleAttributes() -> u64 {
    0
}

pub fn ObSetSecurityDescriptorInfo() -> u64 {
    0
}

pub fn ObSetSecurityObjectByPointer() -> u64 {
    0
}

pub fn ObUnRegisterCallbacks() -> u64 {
    0
}

pub fn ObfDereferenceObject() -> u64 {
    0
}

pub fn ObfReferenceObject() -> u64 {
    0
}

pub fn PfxFindPrefix() -> u64 {
    0
}

pub fn PfxInitialize() -> u64 {
    0
}

pub fn PfxInsertPrefix() -> u64 {
    0
}

pub fn PfxRemovePrefix() -> u64 {
    0
}

pub fn PoCallDriver() -> u64 {
    0
}

pub fn PoCancelDeviceNotify() -> u64 {
    0
}

pub fn PoQueueShutdownWorkItem() -> u64 {
    0
}

pub fn PoRegisterDeviceForIdleDetection() -> u64 {
    0
}

pub fn PoRegisterDeviceNotify() -> u64 {
    0
}

pub fn PoRegisterSystemState() -> u64 {
    0
}

pub fn PoRequestPowerIrp() -> u64 {
    0
}

pub fn PoRequestShutdownEvent() -> u64 {
    0
}

pub fn PoSetHiberRange() -> u64 {
    0
}

pub fn PoSetPowerState() -> u64 {
    0
}

pub fn PoSetSystemState() -> u64 {
    0
}

pub fn PoShutdownBugCheck() -> u64 {
    0
}

pub fn PoStartNextPowerIrp() -> u64 {
    0
}

pub fn PoUnregisterSystemState() -> u64 {
    0
}

pub fn ProbeForRead() -> u64 {
    0
}

pub fn ProbeForWrite() -> u64 {
    0
}

pub fn PsAcquireProcessExitSynchronization() -> u64 {
    0
}

pub fn PsAssignImpersonationToken() -> u64 {
    0
}

pub fn PsChargePoolQuota() -> u64 {
    0
}

pub fn PsChargeProcessNonPagedPoolQuota() -> u64 {
    0
}

pub fn PsChargeProcessPagedPoolQuota() -> u64 {
    0
}

pub fn PsChargeProcessPoolQuota() -> u64 {
    0
}

pub fn PsCreateSystemProcess() -> u64 {
    0
}

pub fn PsCreateSystemThread() -> u64 {
    0
}

pub fn PsDereferenceImpersonationToken() -> u64 {
    0
}

pub fn PsDereferencePrimaryToken() -> u64 {
    0
}

pub fn PsDisableImpersonation() -> u64 {
    0
}

pub fn PsEstablishWin32Callouts() -> u64 {
    0
}

pub fn PsGetContextThread() -> u64 {
    0
}

pub fn PsGetCurrentProcess() -> u64 {
    0
}

pub fn PsGetCurrentProcessId() -> u64 {
    0
}

pub fn PsGetCurrentProcessSessionId() -> u64 {
    0
}

pub fn PsGetCurrentThread() -> u64 {
    0
}

pub fn PsGetCurrentThreadId() -> u64 {
    0
}

pub fn PsGetCurrentThreadPreviousMode() -> u64 {
    0
}

pub fn PsGetCurrentThreadStackBase() -> u64 {
    0
}

pub fn PsGetCurrentThreadStackLimit() -> u64 {
    0
}

pub fn PsGetJobLock() -> u64 {
    0
}

pub fn PsGetJobSessionId() -> u64 {
    0
}

pub fn PsGetJobUIRestrictionsClass() -> u64 {
    0
}

pub fn PsGetProcessCreateTimeQuadPart() -> u64 {
    0
}

pub fn PsGetProcessDebugPort() -> u64 {
    0
}

pub fn PsGetProcessExitProcessCalled() -> u64 {
    0
}

pub fn PsGetProcessExitStatus() -> u64 {
    0
}

pub fn PsGetProcessExitTime() -> u64 {
    0
}

pub fn PsGetProcessId() -> u64 {
    0
}

pub fn PsGetProcessImageFileName() -> u64 {
    0
}

pub fn PsGetProcessInheritedFromUniqueProcessId() -> u64 {
    0
}

pub fn PsGetProcessJob() -> u64 {
    0
}

pub fn PsGetProcessPeb() -> u64 {
    0
}

pub fn PsGetProcessPriorityClass() -> u64 {
    0
}

pub fn PsGetProcessSectionBaseAddress() -> u64 {
    0
}

pub fn PsGetProcessSecurityPort() -> u64 {
    0
}

pub fn PsGetProcessSessionId() -> u64 {
    0
}

pub fn PsGetProcessWin32Process() -> u64 {
    0
}

pub fn PsGetProcessWin32WindowStation() -> u64 {
    0
}

pub fn PsGetThreadFreezeCount() -> u64 {
    0
}

pub fn PsGetThreadHardErrorsAreDisabled() -> u64 {
    0
}

pub fn PsGetThreadId() -> u64 {
    0
}

pub fn PsGetThreadProcess() -> u64 {
    0
}

pub fn PsGetThreadProcessId() -> u64 {
    0
}

pub fn PsGetThreadSessionId() -> u64 {
    0
}

pub fn PsGetThreadTeb() -> u64 {
    0
}

pub fn PsGetThreadWin32Thread() -> u64 {
    0
}

pub fn PsGetVersion() -> u64 {
    0
}

pub fn PsImpersonateClient() -> u64 {
    0
}

pub fn PsIsProcessBeingDebugged() -> u64 {
    0
}

pub fn PsIsSystemThread() -> u64 {
    0
}

pub fn PsIsThreadImpersonating() -> u64 {
    0
}

pub fn PsIsThreadTerminating() -> u64 {
    0
}

pub fn PsJobType() -> u64 {
    0
}

pub fn PsLookupProcessByProcessId() -> u64 {
    0
}

pub fn PsLookupProcessThreadByCid() -> u64 {
    0
}

pub fn PsLookupThreadByThreadId() -> u64 {
    0
}

pub fn PsReferenceImpersonationToken() -> u64 {
    0
}

pub fn PsReferencePrimaryToken() -> u64 {
    0
}

pub fn PsReferenceProcessFilePointer() -> u64 {
    0
}

pub fn PsReleaseProcessExitSynchronization() -> u64 {
    0
}

pub fn PsRemoveCreateThreadNotifyRoutine() -> u64 {
    0
}

pub fn PsRemoveLoadImageNotifyRoutine() -> u64 {
    0
}

pub fn PsRestoreImpersonation() -> u64 {
    0
}

pub fn PsResumeProcess() -> u64 {
    0
}

pub fn PsReturnPoolQuota() -> u64 {
    0
}

pub fn PsReturnProcessNonPagedPoolQuota() -> u64 {
    0
}

pub fn PsReturnProcessPagedPoolQuota() -> u64 {
    0
}

pub fn PsRevertThreadToSelf() -> u64 {
    0
}

pub fn PsRevertToSelf() -> u64 {
    0
}

pub fn PsSetContextThread() -> u64 {
    0
}

pub fn PsSetCreateProcessNotifyRoutine() -> u64 {
    0
}

pub fn PsSetCreateProcessNotifyRoutineEx() -> u64 {
    0
}

pub fn PsSetCreateThreadNotifyRoutine() -> u64 {
    0
}

pub fn PsSetJobUIRestrictionsClass() -> u64 {
    0
}

pub fn PsSetLegoNotifyRoutine() -> u64 {
    0
}

pub fn PsSetLoadImageNotifyRoutine() -> u64 {
    0
}

pub fn PsSetLoadImageNotifyRoutineEx() -> u64 {
    0
}

pub fn PsSetProcessPriorityByClass() -> u64 {
    0
}

pub fn PsSetProcessPriorityClass() -> u64 {
    0
}

pub fn PsSetProcessSecurityPort() -> u64 {
    0
}

pub fn PsSetProcessWin32Process() -> u64 {
    0
}

pub fn PsSetProcessWindowStation() -> u64 {
    0
}

pub fn PsSetThreadHardErrorsAreDisabled() -> u64 {
    0
}

pub fn PsSetThreadWin32Thread() -> u64 {
    0
}

pub fn PsSuspendProcess() -> u64 {
    0
}

pub fn PsTerminateSystemThread() -> u64 {
    0
}

pub fn READ_REGISTER_BUFFER_UCHAR() -> u64 {
    0
}

pub fn READ_REGISTER_BUFFER_ULONG() -> u64 {
    0
}

pub fn READ_REGISTER_BUFFER_USHORT() -> u64 {
    0
}

pub fn READ_REGISTER_UCHAR() -> u64 {
    0
}

pub fn READ_REGISTER_ULONG() -> u64 {
    0
}

pub fn READ_REGISTER_USHORT() -> u64 {
    0
}

pub fn RtlAbsoluteToSelfRelativeSD() -> u64 {
    0
}

pub fn RtlAddAccessAllowedAce() -> u64 {
    0
}

pub fn RtlAddAccessAllowedAceEx() -> u64 {
    0
}

pub fn RtlAddAccessAllowedObjectAce() -> u64 {
    0
}

pub fn RtlAddAccessDeniedAceEx() -> u64 {
    0
}

pub fn RtlAddAccessDeniedObjectAce() -> u64 {
    0
}

pub fn RtlAddAce() -> u64 {
    0
}

pub fn RtlAddAtomToAtomTable() -> u64 {
    0
}

pub fn RtlAddAuditAccessAceEx() -> u64 {
    0
}

pub fn RtlAddAuditAccessObjectAce() -> u64 {
    0
}

pub fn RtlAddRange() -> u64 {
    0
}

pub fn RtlAllocateHeap() -> u64 {
    0
}

pub fn RtlAnsiCharToUnicodeChar() -> u64 {
    0
}

pub fn RtlAnsiStringToUnicodeSize() -> u64 {
    0
}

pub fn RtlAnsiStringToUnicodeString() -> u64 {
    0
}

pub fn RtlAppendAsciizToString() -> u64 {
    0
}

pub fn RtlAppendStringToString() -> u64 {
    0
}

pub fn RtlAppendUnicodeStringToString() -> u64 {
    0
}

pub fn RtlAppendUnicodeToString() -> u64 {
    0
}

pub fn RtlAreAllAccessesGranted() -> u64 {
    0
}

pub fn RtlAreAnyAccessesGranted() -> u64 {
    0
}

pub fn RtlAreBitsClear() -> u64 {
    0
}

pub fn RtlAreBitsSet() -> u64 {
    0
}

pub fn RtlAssert() -> u64 {
    0
}

pub fn RtlCaptureContext() -> u64 {
    0
}

pub fn RtlCaptureStackBackTrace() -> u64 {
    0
}

pub fn RtlCharToInteger() -> u64 {
    0
}

pub fn RtlCheckRegistryKey() -> u64 {
    0
}

pub fn RtlClearAllBits() -> u64 {
    0
}

pub fn RtlClearBit() -> u64 {
    0
}

pub fn RtlClearBits() -> u64 {
    0
}

pub fn RtlCompareMemory() -> u64 {
    0
}

pub fn RtlCompareMemoryUlong() -> u64 {
    0
}

pub fn RtlCompareString() -> u64 {
    0
}

pub fn RtlCompareUnicodeString() -> u64 {
    0
}

pub fn RtlCompareUnicodeStrings() -> u64 {
    0
}

pub fn RtlCompressBuffer() -> u64 {
    0
}

pub fn RtlCompressChunks() -> u64 {
    0
}

pub fn RtlComputeCrc32() -> u64 {
    0
}

pub fn RtlConvertSidToUnicodeString() -> u64 {
    0
}

pub fn RtlCopyExtendedContext() -> u64 {
    0
}

pub fn RtlCopyLuid() -> u64 {
    0
}

pub fn RtlCopyLuidAndAttributesArray() -> u64 {
    0
}

pub fn RtlCopyRangeList() -> u64 {
    0
}

pub fn RtlCopySid() -> u64 {
    0
}

pub fn RtlCopyString() -> u64 {
    0
}

pub fn RtlCopyUnicodeString() -> u64 {
    0
}

pub fn RtlCreateAcl() -> u64 {
    0
}

pub fn RtlCreateAtomTable() -> u64 {
    0
}

pub fn RtlCreateHeap() -> u64 {
    0
}

pub fn RtlCreateRegistryKey() -> u64 {
    0
}

pub fn RtlCreateSecurityDescriptor() -> u64 {
    0
}

pub fn RtlCreateSystemVolumeInformationFolder() -> u64 {
    0
}

pub fn RtlCreateUnicodeString() -> u64 {
    0
}

pub fn RtlCreateUserThread() -> u64 {
    0
}

pub fn RtlCustomCPToUnicodeN() -> u64 {
    0
}

pub fn RtlDecompressBuffer() -> u64 {
    0
}

pub fn RtlDecompressChunks() -> u64 {
    0
}

pub fn RtlDecompressFragment() -> u64 {
    0
}

pub fn RtlDelete() -> u64 {
    0
}

pub fn RtlDeleteAce() -> u64 {
    0
}

pub fn RtlDeleteAtomFromAtomTable() -> u64 {
    0
}

pub fn RtlDeleteElementGenericTable() -> u64 {
    0
}

pub fn RtlDeleteElementGenericTableAvl() -> u64 {
    0
}

pub fn RtlDeleteNoSplay() -> u64 {
    0
}

pub fn RtlDeleteOwnersRanges() -> u64 {
    0
}

pub fn RtlDeleteRange() -> u64 {
    0
}

pub fn RtlDeleteRegistryValue() -> u64 {
    0
}

pub fn RtlDescribeChunk() -> u64 {
    0
}

pub fn RtlDestroyAtomTable() -> u64 {
    0
}

pub fn RtlDestroyHeap() -> u64 {
    0
}

pub fn RtlDowncaseUnicodeChar() -> u64 {
    0
}

pub fn RtlDowncaseUnicodeString() -> u64 {
    0
}

pub fn RtlDuplicateUnicodeString() -> u64 {
    0
}

pub fn RtlEmptyAtomTable() -> u64 {
    0
}

pub fn RtlEnumerateGenericTable() -> u64 {
    0
}

pub fn RtlEnumerateGenericTableAvl() -> u64 {
    0
}

pub fn RtlEnumerateGenericTableLikeADirectory() -> u64 {
    0
}

pub fn RtlEnumerateGenericTableWithoutSplaying() -> u64 {
    0
}

pub fn RtlEnumerateGenericTableWithoutSplayingAvl() -> u64 {
    0
}

pub fn RtlEqualLuid() -> u64 {
    0
}

pub fn RtlEqualSid() -> u64 {
    0
}

pub fn RtlEqualString() -> u64 {
    0
}

pub fn RtlEqualUnicodeString() -> u64 {
    0
}

pub fn RtlFillMemory() -> u64 {
    0
}

pub fn RtlFillMemoryUlong() -> u64 {
    0
}

pub fn RtlFindClearBits() -> u64 {
    0
}

pub fn RtlFindClearBitsAndSet() -> u64 {
    0
}

pub fn RtlFindClearRuns() -> u64 {
    0
}

pub fn RtlFindFirstRunClear() -> u64 {
    0
}

pub fn RtlFindLastBackwardRunClear() -> u64 {
    0
}

pub fn RtlFindLeastSignificantBit() -> u64 {
    0
}

pub fn RtlFindLongestRunClear() -> u64 {
    0
}

pub fn RtlFindMessage() -> u64 {
    0
}

pub fn RtlFindMostSignificantBit() -> u64 {
    0
}

pub fn RtlFindNextForwardRunClear() -> u64 {
    0
}

pub fn RtlFindRange() -> u64 {
    0
}

pub fn RtlFindSetBits() -> u64 {
    0
}

pub fn RtlFindSetBitsAndClear() -> u64 {
    0
}

pub fn RtlFindUnicodePrefix() -> u64 {
    0
}

pub fn RtlFirstFreeAce() -> u64 {
    0
}

pub fn RtlFormatCurrentUserKeyPath() -> u64 {
    0
}

pub fn RtlFormatMessage() -> u64 {
    0
}

pub fn RtlFormatMessageEx() -> u64 {
    0
}

pub fn RtlFreeAnsiString() -> u64 {
    0
}

pub fn RtlFreeHeap() -> u64 {
    0
}

pub fn RtlFreeOemString() -> u64 {
    0
}

pub fn RtlFreeRangeList() -> u64 {
    0
}

pub fn RtlFreeUnicodeString() -> u64 {
    0
}

pub fn RtlGUIDFromString() -> u64 {
    0
}

pub fn RtlGenerate8dot3Name() -> u64 {
    0
}

pub fn RtlGetAce() -> u64 {
    0
}

pub fn RtlGetCallersAddress() -> u64 {
    0
}

pub fn RtlGetCompressionWorkSpaceSize() -> u64 {
    0
}

pub fn RtlGetControlSecurityDescriptor() -> u64 {
    0
}

pub fn RtlGetDaclSecurityDescriptor() -> u64 {
    0
}

pub fn RtlGetDefaultCodePage() -> u64 {
    0
}

pub fn RtlGetElementGenericTable() -> u64 {
    0
}

pub fn RtlGetElementGenericTableAvl() -> u64 {
    0
}

pub fn RtlGetEnabledExtendedFeatures() -> u64 {
    0
}

pub fn RtlGetExtendedContextLength() -> u64 {
    0
}

pub fn RtlGetExtendedContextLength2() -> u64 {
    0
}

pub fn RtlGetExtendedFeaturesMask() -> u64 {
    0
}

pub fn RtlGetFirstRange() -> u64 {
    0
}

pub fn RtlGetGroupSecurityDescriptor() -> u64 {
    0
}

pub fn RtlGetNextRange() -> u64 {
    0
}

pub fn RtlGetNtGlobalFlags() -> u64 {
    0
}

pub fn RtlGetNtProductType() -> u64 {
    0
}

pub fn RtlGetOwnerSecurityDescriptor() -> u64 {
    0
}

pub fn RtlGetProductInfo() -> u64 {
    0
}

pub fn RtlGetSaclSecurityDescriptor() -> u64 {
    0
}

pub fn RtlGetSetBootStatusData() -> u64 {
    0
}

pub fn RtlGetVersion() -> u64 {
    0
}

pub fn RtlHashUnicodeString() -> u64 {
    0
}

pub fn RtlIdnToAscii() -> u64 {
    0
}

pub fn RtlIdnToNameprepUnicode() -> u64 {
    0
}

pub fn RtlIdnToUnicode() -> u64 {
    0
}

pub fn RtlImageDirectoryEntryToData() -> u64 {
    0
}

pub fn RtlImageNtHeader() -> u64 {
    0
}

pub fn RtlInitAnsiString() -> u64 {
    0
}

pub fn RtlInitAnsiStringEx() -> u64 {
    0
}

pub fn RtlInitCodePageTable() -> u64 {
    0
}

pub fn RtlInitString() -> u64 {
    0
}

pub fn RtlInitUnicodeString() -> u64 {
    0
}

pub fn RtlInitUnicodeStringEx() -> u64 {
    0
}

pub fn RtlInitializeBitMap() -> u64 {
    0
}

pub fn RtlInitializeExtendedContext() -> u64 {
    0
}

pub fn RtlInitializeExtendedContext2() -> u64 {
    0
}

pub fn RtlInitializeGenericTable() -> u64 {
    0
}

pub fn RtlInitializeGenericTableAvl() -> u64 {
    0
}

pub fn RtlInitializeRangeList() -> u64 {
    0
}

pub fn RtlInitializeSid() -> u64 {
    0
}

pub fn RtlInitializeUnicodePrefix() -> u64 {
    0
}

pub fn RtlInsertElementGenericTable() -> u64 {
    0
}

pub fn RtlInsertElementGenericTableAvl() -> u64 {
    0
}

pub fn RtlInsertElementGenericTableFull() -> u64 {
    0
}

pub fn RtlInsertElementGenericTableFullAvl() -> u64 {
    0
}

pub fn RtlInsertUnicodePrefix() -> u64 {
    0
}

pub fn RtlInt64ToUnicodeString() -> u64 {
    0
}

pub fn RtlIntegerToChar() -> u64 {
    0
}

pub fn RtlIntegerToUnicode() -> u64 {
    0
}

pub fn RtlIntegerToUnicodeString() -> u64 {
    0
}

pub fn RtlInvertRangeList() -> u64 {
    0
}

pub fn RtlIpv4AddressToStringA() -> u64 {
    0
}

pub fn RtlIpv4AddressToStringExA() -> u64 {
    0
}

pub fn RtlIpv4AddressToStringExW() -> u64 {
    0
}

pub fn RtlIpv4AddressToStringW() -> u64 {
    0
}

pub fn RtlIpv4StringToAddressA() -> u64 {
    0
}

pub fn RtlIpv4StringToAddressExA() -> u64 {
    0
}

pub fn RtlIpv4StringToAddressExW() -> u64 {
    0
}

pub fn RtlIpv4StringToAddressW() -> u64 {
    0
}

pub fn RtlIpv6AddressToStringA() -> u64 {
    0
}

pub fn RtlIpv6AddressToStringExA() -> u64 {
    0
}

pub fn RtlIpv6AddressToStringExW() -> u64 {
    0
}

pub fn RtlIpv6AddressToStringW() -> u64 {
    0
}

pub fn RtlIpv6StringToAddressA() -> u64 {
    0
}

pub fn RtlIpv6StringToAddressExA() -> u64 {
    0
}

pub fn RtlIpv6StringToAddressExW() -> u64 {
    0
}

pub fn RtlIpv6StringToAddressW() -> u64 {
    0
}

pub fn RtlIsGenericTableEmpty() -> u64 {
    0
}

pub fn RtlIsGenericTableEmptyAvl() -> u64 {
    0
}

pub fn RtlIsNameLegalDOS8Dot3() -> u64 {
    0
}

pub fn RtlIsNormalizedString() -> u64 {
    0
}

pub fn RtlIsNtDdiVersionAvailable() -> u64 {
    0
}

pub fn RtlIsRangeAvailable() -> u64 {
    0
}

pub fn RtlIsValidOemCharacter() -> u64 {
    0
}

pub fn RtlLargeIntegerToChar() -> u64 {
    0
}

pub fn RtlLengthRequiredSid() -> u64 {
    0
}

pub fn RtlLengthSecurityDescriptor() -> u64 {
    0
}

pub fn RtlLengthSid() -> u64 {
    0
}

pub fn RtlLocalTimeToSystemTime() -> u64 {
    0
}

pub fn RtlLocateExtendedFeature() -> u64 {
    0
}

pub fn RtlLocateExtendedFeature2() -> u64 {
    0
}

pub fn RtlLocateLegacyContext() -> u64 {
    0
}

pub fn RtlLockBootStatusData() -> u64 {
    0
}

pub fn RtlLookupAtomInAtomTable() -> u64 {
    0
}

pub fn RtlLookupElementGenericTable() -> u64 {
    0
}

pub fn RtlLookupElementGenericTableAvl() -> u64 {
    0
}

pub fn RtlLookupElementGenericTableFull() -> u64 {
    0
}

pub fn RtlLookupElementGenericTableFullAvl() -> u64 {
    0
}

pub fn RtlMapGenericMask() -> u64 {
    0
}

pub fn RtlMapSecurityErrorToNtStatus() -> u64 {
    0
}

pub fn RtlMergeRangeLists() -> u64 {
    0
}

pub fn RtlMoveMemory() -> u64 {
    0
}

pub fn RtlMultiByteToUnicodeN() -> u64 {
    0
}

pub fn RtlMultiByteToUnicodeSize() -> u64 {
    0
}

pub fn RtlNextUnicodePrefix() -> u64 {
    0
}

pub fn RtlNormalizeString() -> u64 {
    0
}

pub fn RtlNtStatusToDosError() -> u64 {
    0
}

pub fn RtlNtStatusToDosErrorNoTeb() -> u64 {
    0
}

pub fn RtlNumberGenericTableElements() -> u64 {
    0
}

pub fn RtlNumberGenericTableElementsAvl() -> u64 {
    0
}

pub fn RtlNumberOfClearBits() -> u64 {
    0
}

pub fn RtlNumberOfSetBits() -> u64 {
    0
}

pub fn RtlOemStringToCountedUnicodeString() -> u64 {
    0
}

pub fn RtlOemStringToUnicodeSize() -> u64 {
    0
}

pub fn RtlOemStringToUnicodeString() -> u64 {
    0
}

pub fn RtlOemToUnicodeN() -> u64 {
    0
}

pub fn RtlOpenCurrentUser() -> u64 {
    0
}

pub fn RtlPcToFileHeader() -> u64 {
    0
}

pub fn RtlPinAtomInAtomTable() -> u64 {
    0
}

pub fn RtlPrefetchMemoryNonTemporal() -> u64 {
    0
}

pub fn RtlPrefixString() -> u64 {
    0
}

pub fn RtlPrefixUnicodeString() -> u64 {
    0
}

pub fn RtlQueryAtomInAtomTable() -> u64 {
    0
}

pub fn RtlQueryDynamicTimeZoneInformation() -> u64 {
    0
}

pub fn RtlQueryInformationAcl() -> u64 {
    0
}

pub fn RtlQueryPackageIdentity() -> u64 {
    0
}

pub fn RtlQueryProcessPlaceholderCompatibilityMode() -> u64 {
    0
}

pub fn RtlQueryRegistryValues() -> u64 {
    0
}

pub fn RtlQueryRegistryValuesEx() -> u64 {
    0
}

pub fn RtlQueryTimeZoneInformation() -> u64 {
    0
}

pub fn RtlRaiseException() -> u64 {
    0
}

pub fn RtlRaiseStatus() -> u64 {
    0
}

pub fn RtlRandom() -> u64 {
    0
}

pub fn RtlRandomEx() -> u64 {
    0
}

pub fn RtlRealPredecessor() -> u64 {
    0
}

pub fn RtlRealSuccessor() -> u64 {
    0
}

pub fn RtlRemoveUnicodePrefix() -> u64 {
    0
}

pub fn RtlReserveChunk() -> u64 {
    0
}

pub fn RtlRunOnceBeginInitialize() -> u64 {
    0
}

pub fn RtlRunOnceComplete() -> u64 {
    0
}

pub fn RtlRunOnceExecuteOnce() -> u64 {
    0
}

pub fn RtlRunOnceInitialize() -> u64 {
    0
}

pub fn RtlSecondsSince1970ToTime() -> u64 {
    0
}

pub fn RtlSecondsSince1980ToTime() -> u64 {
    0
}

pub fn RtlSelfRelativeToAbsoluteSD() -> u64 {
    0
}

pub fn RtlSelfRelativeToAbsoluteSD2() -> u64 {
    0
}

pub fn RtlSetAllBits() -> u64 {
    0
}

pub fn RtlSetBit() -> u64 {
    0
}

pub fn RtlSetBits() -> u64 {
    0
}

pub fn RtlSetControlSecurityDescriptor() -> u64 {
    0
}

pub fn RtlSetDaclSecurityDescriptor() -> u64 {
    0
}

pub fn RtlSetExtendedFeaturesMask() -> u64 {
    0
}

pub fn RtlSetGroupSecurityDescriptor() -> u64 {
    0
}

pub fn RtlSetOwnerSecurityDescriptor() -> u64 {
    0
}

pub fn RtlSetSaclSecurityDescriptor() -> u64 {
    0
}

pub fn RtlSetTimeZoneInformation() -> u64 {
    0
}

pub fn RtlSizeHeap() -> u64 {
    0
}

pub fn RtlSplay() -> u64 {
    0
}

pub fn RtlStringFromGUID() -> u64 {
    0
}

pub fn RtlSubAuthorityCountSid() -> u64 {
    0
}

pub fn RtlSubAuthoritySid() -> u64 {
    0
}

pub fn RtlSubtreePredecessor() -> u64 {
    0
}

pub fn RtlSubtreeSuccessor() -> u64 {
    0
}

pub fn RtlSystemTimeToLocalTime() -> u64 {
    0
}

pub fn RtlTestBit() -> u64 {
    0
}

pub fn RtlTimeFieldsToTime() -> u64 {
    0
}

pub fn RtlTimeToElapsedTimeFields() -> u64 {
    0
}

pub fn RtlTimeToSecondsSince1970() -> u64 {
    0
}

pub fn RtlTimeToSecondsSince1980() -> u64 {
    0
}

pub fn RtlTimeToTimeFields() -> u64 {
    0
}

pub fn RtlTraceDatabaseAdd() -> u64 {
    0
}

pub fn RtlTraceDatabaseCreate() -> u64 {
    0
}

pub fn RtlTraceDatabaseDestroy() -> u64 {
    0
}

pub fn RtlTraceDatabaseEnumerate() -> u64 {
    0
}

pub fn RtlTraceDatabaseFind() -> u64 {
    0
}

pub fn RtlTraceDatabaseLock() -> u64 {
    0
}

pub fn RtlTraceDatabaseUnlock() -> u64 {
    0
}

pub fn RtlTraceDatabaseValidate() -> u64 {
    0
}

pub fn RtlUTF8ToUnicodeN() -> u64 {
    0
}

pub fn RtlUnicodeStringToAnsiSize() -> u64 {
    0
}

pub fn RtlUnicodeStringToAnsiString() -> u64 {
    0
}

pub fn RtlUnicodeStringToCountedOemString() -> u64 {
    0
}

pub fn RtlUnicodeStringToInteger() -> u64 {
    0
}

pub fn RtlUnicodeStringToOemSize() -> u64 {
    0
}

pub fn RtlUnicodeStringToOemString() -> u64 {
    0
}

pub fn RtlUnicodeToCustomCPN() -> u64 {
    0
}

pub fn RtlUnicodeToMultiByteN() -> u64 {
    0
}

pub fn RtlUnicodeToMultiByteSize() -> u64 {
    0
}

pub fn RtlUnicodeToOemN() -> u64 {
    0
}

pub fn RtlUnicodeToUTF8N() -> u64 {
    0
}

pub fn RtlUnlockBootStatusData() -> u64 {
    0
}

pub fn RtlUnwind() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeChar() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeString() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeStringToAnsiString() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeStringToCountedOemString() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeStringToOemString() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeToCustomCPN() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeToMultiByteN() -> u64 {
    0
}

pub fn RtlUpcaseUnicodeToOemN() -> u64 {
    0
}

pub fn RtlUpperChar() -> u64 {
    0
}

pub fn RtlUpperString() -> u64 {
    0
}

pub fn RtlValidAcl() -> u64 {
    0
}

pub fn RtlValidRelativeSecurityDescriptor() -> u64 {
    0
}

pub fn RtlValidSecurityDescriptor() -> u64 {
    0
}

pub fn RtlValidSid() -> u64 {
    0
}

pub fn RtlVerifyVersionInfo() -> u64 {
    0
}

pub fn RtlVolumeDeviceToDosName() -> u64 {
    0
}

pub fn RtlWalkFrameChain() -> u64 {
    0
}

pub fn RtlWriteRegistryValue() -> u64 {
    0
}

pub fn RtlZeroHeap() -> u64 {
    0
}

pub fn RtlZeroMemory() -> u64 {
    0
}

pub fn RtlxAnsiStringToUnicodeSize() -> u64 {
    0
}

pub fn RtlxOemStringToUnicodeSize() -> u64 {
    0
}

pub fn RtlxUnicodeStringToAnsiSize() -> u64 {
    0
}

pub fn RtlxUnicodeStringToOemSize() -> u64 {
    0
}

pub fn SeAccessCheck() -> u64 {
    0
}

pub fn SeAppendPrivileges() -> u64 {
    0
}

pub fn SeAssignSecurity() -> u64 {
    0
}

pub fn SeAssignSecurityEx() -> u64 {
    0
}

pub fn SeAuditHardLinkCreation() -> u64 {
    0
}

pub fn SeAuditingFileEvents() -> u64 {
    0
}

pub fn SeAuditingFileEventsWithContext() -> u64 {
    0
}

pub fn SeAuditingFileOrGlobalEvents() -> u64 {
    0
}

pub fn SeAuditingHardLinkEvents() -> u64 {
    0
}

pub fn SeAuditingHardLinkEventsWithContext() -> u64 {
    0
}

pub fn SeCaptureSecurityDescriptor() -> u64 {
    0
}

pub fn SeCaptureSubjectContext() -> u64 {
    0
}

pub fn SeCloseObjectAuditAlarm() -> u64 {
    0
}

pub fn SeCreateAccessState() -> u64 {
    0
}

pub fn SeCreateClientSecurity() -> u64 {
    0
}

pub fn SeCreateClientSecurityFromSubjectContext() -> u64 {
    0
}

pub fn SeDeassignSecurity() -> u64 {
    0
}

pub fn SeDeleteAccessState() -> u64 {
    0
}

pub fn SeDeleteObjectAuditAlarm() -> u64 {
    0
}

pub fn SeExports() -> u64 {
    0
}

pub fn SeFilterToken() -> u64 {
    0
}

pub fn SeFreePrivileges() -> u64 {
    0
}

pub fn SeImpersonateClient() -> u64 {
    0
}

pub fn SeImpersonateClientEx() -> u64 {
    0
}

pub fn SeLocateProcessImageName() -> u64 {
    0
}

pub fn SeLockSubjectContext() -> u64 {
    0
}

pub fn SeMarkLogonSessionForTerminationNotification() -> u64 {
    0
}

pub fn SeOpenObjectAuditAlarm() -> u64 {
    0
}

pub fn SeOpenObjectForDeleteAuditAlarm() -> u64 {
    0
}

pub fn SePrivilegeCheck() -> u64 {
    0
}

pub fn SePrivilegeObjectAuditAlarm() -> u64 {
    0
}

pub fn SePublicDefaultDacl() -> u64 {
    0
}

pub fn SeQueryAuthenticationIdToken() -> u64 {
    0
}

pub fn SeQueryInformationToken() -> u64 {
    0
}

pub fn SeQuerySecurityDescriptorInfo() -> u64 {
    0
}

pub fn SeQuerySessionIdToken() -> u64 {
    0
}

pub fn SeRegisterLogonSessionTerminatedRoutine() -> u64 {
    0
}

pub fn SeReleaseSecurityDescriptor() -> u64 {
    0
}

pub fn SeReleaseSubjectContext() -> u64 {
    0
}

pub fn SeSetAccessStateGenericMapping() -> u64 {
    0
}

pub fn SeSetSecurityDescriptorInfo() -> u64 {
    0
}

pub fn SeSetSecurityDescriptorInfoEx() -> u64 {
    0
}

pub fn SeSinglePrivilegeCheck() -> u64 {
    0
}

pub fn SeSystemDefaultDacl() -> u64 {
    0
}

pub fn SeTokenImpersonationLevel() -> u64 {
    0
}

pub fn SeTokenIsAdmin() -> u64 {
    0
}

pub fn SeTokenIsRestricted() -> u64 {
    0
}

pub fn SeTokenIsWriteRestricted() -> u64 {
    0
}

pub fn SeTokenType() -> u64 {
    0
}

pub fn SeUnlockSubjectContext() -> u64 {
    0
}

pub fn SeUnregisterLogonSessionTerminatedRoutine() -> u64 {
    0
}

pub fn SeValidSecurityDescriptor() -> u64 {
    0
}

pub fn VerSetConditionMask() -> u64 {
    0
}

pub fn VfFailDeviceNode() -> u64 {
    0
}

pub fn VfFailDriver() -> u64 {
    0
}

pub fn VfFailSystemBIOS() -> u64 {
    0
}

pub fn VfIsVerificationEnabled() -> u64 {
    0
}

pub fn WRITE_REGISTER_BUFFER_UCHAR() -> u64 {
    0
}

pub fn WRITE_REGISTER_BUFFER_ULONG() -> u64 {
    0
}

pub fn WRITE_REGISTER_BUFFER_USHORT() -> u64 {
    0
}

pub fn WRITE_REGISTER_UCHAR() -> u64 {
    0
}

pub fn WRITE_REGISTER_ULONG() -> u64 {
    0
}

pub fn WRITE_REGISTER_USHORT() -> u64 {
    0
}

pub fn WmiFlushTrace() -> u64 {
    0
}

pub fn WmiGetClock() -> u64 {
    0
}

pub fn WmiQueryTrace() -> u64 {
    0
}

pub fn WmiQueryTraceInformation() -> u64 {
    0
}

pub fn WmiStartTrace() -> u64 {
    0
}

pub fn WmiStopTrace() -> u64 {
    0
}

pub fn WmiTraceMessage() -> u64 {
    0
}

pub fn WmiTraceMessageVa() -> u64 {
    0
}

pub fn WmiUpdateTrace() -> u64 {
    0
}

pub fn XIPDispatch() -> u64 {
    0
}

pub fn ZwAccessCheckAndAuditAlarm() -> u64 {
    0
}

pub fn ZwAddBootEntry() -> u64 {
    0
}

pub fn ZwAdjustPrivilegesToken() -> u64 {
    0
}

pub fn ZwAlertThread() -> u64 {
    0
}

pub fn ZwAllocateLocallyUniqueId() -> u64 {
    0
}

pub fn ZwAllocateVirtualMemory() -> u64 {
    0
}

pub fn ZwAssignProcessToJobObject() -> u64 {
    0
}

pub fn ZwCancelIoFile() -> u64 {
    0
}

pub fn ZwCancelIoFileEx() -> u64 {
    0
}

pub fn ZwCancelTimer() -> u64 {
    0
}

pub fn ZwClearEvent() -> u64 {
    0
}

pub fn ZwClose() -> u64 {
    0
}

pub fn ZwCloseObjectAuditAlarm() -> u64 {
    0
}

pub fn ZwConnectPort() -> u64 {
    0
}

pub fn ZwCreateDirectoryObject() -> u64 {
    0
}

pub fn ZwCreateEvent() -> u64 {
    0
}

pub fn ZwCreateFile() -> u64 {
    0
}

pub fn ZwCreateIoCompletion() -> u64 {
    0
}

pub fn ZwCreateJobObject() -> u64 {
    0
}

pub fn ZwCreateKey() -> u64 {
    0
}

pub fn ZwCreateKeyTransacted() -> u64 {
    0
}

pub fn ZwCreateSection() -> u64 {
    0
}

pub fn ZwCreateSymbolicLinkObject() -> u64 {
    0
}

pub fn ZwCreateTimer() -> u64 {
    0
}

pub fn ZwDeleteBootEntry() -> u64 {
    0
}

pub fn ZwDeleteFile() -> u64 {
    0
}

pub fn ZwDeleteKey() -> u64 {
    0
}

pub fn ZwDeleteValueKey() -> u64 {
    0
}

pub fn ZwDeviceIoControlFile() -> u64 {
    0
}

pub fn ZwDisplayString() -> u64 {
    0
}

pub fn ZwDuplicateObject() -> u64 {
    0
}

pub fn ZwDuplicateToken() -> u64 {
    0
}

pub fn ZwEnumerateBootEntries() -> u64 {
    0
}

pub fn ZwEnumerateKey() -> u64 {
    0
}

pub fn ZwEnumerateValueKey() -> u64 {
    0
}

pub fn ZwFlushBuffersFile() -> u64 {
    0
}

pub fn ZwFlushInstructionCache() -> u64 {
    0
}

pub fn ZwFlushKey() -> u64 {
    0
}

pub fn ZwFlushVirtualMemory() -> u64 {
    0
}

pub fn ZwFreeVirtualMemory() -> u64 {
    0
}

pub fn ZwFsControlFile() -> u64 {
    0
}

pub fn ZwImpersonateAnonymousToken() -> u64 {
    0
}

pub fn ZwInitiatePowerAction() -> u64 {
    0
}

pub fn ZwIsProcessInJob() -> u64 {
    0
}

pub fn ZwLoadDriver() -> u64 {
    0
}

pub fn ZwLoadKey() -> u64 {
    0
}

pub fn ZwLockFile() -> u64 {
    0
}

pub fn ZwLockVirtualMemory() -> u64 {
    0
}

pub fn ZwMakePermanentObject() -> u64 {
    0
}

pub fn ZwMakeTemporaryObject() -> u64 {
    0
}

pub fn ZwMapViewOfSection() -> u64 {
    0
}

pub fn ZwNotifyChangeDirectoryFile() -> u64 {
    0
}

pub fn ZwNotifyChangeKey() -> u64 {
    0
}

pub fn ZwOpenDirectoryObject() -> u64 {
    0
}

pub fn ZwOpenEvent() -> u64 {
    0
}

pub fn ZwOpenFile() -> u64 {
    0
}

pub fn ZwOpenJobObject() -> u64 {
    0
}

pub fn ZwOpenKey() -> u64 {
    0
}

pub fn ZwOpenKeyEx() -> u64 {
    0
}

pub fn ZwOpenKeyTransacted() -> u64 {
    0
}

pub fn ZwOpenKeyTransactedEx() -> u64 {
    0
}

pub fn ZwOpenProcess() -> u64 {
    0
}

pub fn ZwOpenProcessToken() -> u64 {
    0
}

pub fn ZwOpenProcessTokenEx() -> u64 {
    0
}

pub fn ZwOpenSection() -> u64 {
    0
}

pub fn ZwOpenSymbolicLinkObject() -> u64 {
    0
}

pub fn ZwOpenThread() -> u64 {
    0
}

pub fn ZwOpenThreadToken() -> u64 {
    0
}

pub fn ZwOpenThreadTokenEx() -> u64 {
    0
}

pub fn ZwOpenTimer() -> u64 {
    0
}

pub fn ZwPowerInformation() -> u64 {
    0
}

pub fn ZwProtectVirtualMemory() -> u64 {
    0
}

pub fn ZwPulseEvent() -> u64 {
    0
}

pub fn ZwQueryBootEntryOrder() -> u64 {
    0
}

pub fn ZwQueryBootOptions() -> u64 {
    0
}

pub fn ZwQueryDefaultLocale() -> u64 {
    0
}

pub fn ZwQueryDefaultUILanguage() -> u64 {
    0
}

pub fn ZwQueryDirectoryFile() -> u64 {
    0
}

pub fn ZwQueryDirectoryObject() -> u64 {
    0
}

pub fn ZwQueryEaFile() -> u64 {
    0
}

pub fn ZwQueryFullAttributesFile() -> u64 {
    0
}

pub fn ZwQueryInformationFile() -> u64 {
    0
}

pub fn ZwQueryInformationJobObject() -> u64 {
    0
}

pub fn ZwQueryInformationProcess() -> u64 {
    0
}

pub fn ZwQueryInformationThread() -> u64 {
    0
}

pub fn ZwQueryInformationToken() -> u64 {
    0
}

pub fn ZwQueryInstallUILanguage() -> u64 {
    0
}

pub fn ZwQueryKey() -> u64 {
    0
}

pub fn ZwQueryLicenseValue() -> u64 {
    0
}

pub fn ZwQueryObject() -> u64 {
    0
}

pub fn ZwQuerySection() -> u64 {
    0
}

pub fn ZwQuerySecurityObject() -> u64 {
    0
}

pub fn ZwQuerySymbolicLinkObject() -> u64 {
    0
}

pub fn ZwQuerySystemEnvironmentValueEx() -> u64 {
    0
}

pub fn ZwQuerySystemInformation() -> u64 {
    0
}

pub fn ZwQuerySystemInformationEx() -> u64 {
    0
}

pub fn ZwQueryTimerResolution() -> u64 {
    0
}

pub fn ZwQueryValueKey() -> u64 {
    0
}

pub fn ZwQueryVirtualMemory() -> u64 {
    0
}

pub fn ZwQueryVolumeInformationFile() -> u64 {
    0
}

pub fn ZwReadFile() -> u64 {
    0
}

pub fn ZwRemoveIoCompletion() -> u64 {
    0
}

pub fn ZwRemoveIoCompletionEx() -> u64 {
    0
}

pub fn ZwRenameKey() -> u64 {
    0
}

pub fn ZwReplaceKey() -> u64 {
    0
}

pub fn ZwRequestWaitReplyPort() -> u64 {
    0
}

pub fn ZwResetEvent() -> u64 {
    0
}

pub fn ZwRestoreKey() -> u64 {
    0
}

pub fn ZwSaveKey() -> u64 {
    0
}

pub fn ZwSaveKeyEx() -> u64 {
    0
}

pub fn ZwSecureConnectPort() -> u64 {
    0
}

pub fn ZwSetBootEntryOrder() -> u64 {
    0
}

pub fn ZwSetBootOptions() -> u64 {
    0
}

pub fn ZwSetDefaultLocale() -> u64 {
    0
}

pub fn ZwSetDefaultUILanguage() -> u64 {
    0
}

pub fn ZwSetEaFile() -> u64 {
    0
}

pub fn ZwSetEvent() -> u64 {
    0
}

pub fn ZwSetEventBoostPriority() -> u64 {
    0
}

pub fn ZwSetInformationFile() -> u64 {
    0
}

pub fn ZwSetInformationJobObject() -> u64 {
    0
}

pub fn ZwSetInformationKey() -> u64 {
    0
}

pub fn ZwSetInformationObject() -> u64 {
    0
}

pub fn ZwSetInformationProcess() -> u64 {
    0
}

pub fn ZwSetInformationThread() -> u64 {
    0
}

pub fn ZwSetInformationToken() -> u64 {
    0
}

pub fn ZwSetIntervalProfile() -> u64 {
    0
}

pub fn ZwSetIoCompletion() -> u64 {
    0
}

pub fn ZwSetIoCompletionEx() -> u64 {
    0
}

pub fn ZwSetSecurityObject() -> u64 {
    0
}

pub fn ZwSetSystemInformation() -> u64 {
    0
}

pub fn ZwSetSystemTime() -> u64 {
    0
}

pub fn ZwSetTimer() -> u64 {
    0
}

pub fn ZwSetTimerResolution() -> u64 {
    0
}

pub fn ZwSetValueKey() -> u64 {
    0
}

pub fn ZwSetVolumeInformationFile() -> u64 {
    0
}

pub fn ZwTerminateJobObject() -> u64 {
    0
}

pub fn ZwTerminateProcess() -> u64 {
    0
}

pub fn ZwTraceControl() -> u64 {
    0
}

pub fn ZwTranslateFilePath() -> u64 {
    0
}

pub fn ZwUnloadDriver() -> u64 {
    0
}

pub fn ZwUnloadKey() -> u64 {
    0
}

pub fn ZwUnlockFile() -> u64 {
    0
}

pub fn ZwUnlockVirtualMemory() -> u64 {
    0
}

pub fn ZwUnmapViewOfSection() -> u64 {
    0
}

pub fn ZwWaitForMultipleObjects() -> u64 {
    0
}

pub fn ZwWaitForSingleObject() -> u64 {
    0
}

pub fn ZwWriteFile() -> u64 {
    0
}

pub fn ZwYieldExecution() -> u64 {
    0
}

pub fn _abnormal_termination() -> u64 {
    0
}

pub fn _finite() -> u64 {
    0
}

pub fn _i64toa_s() -> u64 {
    0
}

pub fn _i64tow_s() -> u64 {
    0
}

pub fn _itoa() -> u64 {
    0
}

pub fn _itoa_s() -> u64 {
    0
}

pub fn _itow() -> u64 {
    0
}

pub fn _itow_s() -> u64 {
    0
}

pub fn _ltoa_s() -> u64 {
    0
}

pub fn _ltow_s() -> u64 {
    0
}

pub fn _makepath_s() -> u64 {
    0
}

pub fn _purecall() -> u64 {
    0
}

pub fn _setjmp() -> u64 {
    0
}

pub fn _splitpath_s() -> u64 {
    0
}

pub fn _stricmp() -> u64 {
    0
}

pub fn _strlwr() -> u64 {
    0
}

pub fn _strnicmp() -> u64 {
    0
}

pub fn _strnset() -> u64 {
    0
}

pub fn _strnset_s() -> u64 {
    0
}

pub fn _strrev() -> u64 {
    0
}

pub fn _strset() -> u64 {
    0
}

pub fn _strtoui64() -> u64 {
    0
}

pub fn _strupr() -> u64 {
    0
}

pub fn _ui64toa_s() -> u64 {
    0
}

pub fn _ui64tow_s() -> u64 {
    0
}

pub fn _ultoa_s() -> u64 {
    0
}

pub fn _ultow_s() -> u64 {
    0
}

pub fn _vsnprintf() -> u64 {
    0
}

pub fn _vsnprintf_s() -> u64 {
    0
}

pub fn _vsnwprintf() -> u64 {
    0
}

pub fn _vsnwprintf_s() -> u64 {
    0
}

pub fn _vswprintf() -> u64 {
    0
}

pub fn _wcsicmp() -> u64 {
    0
}

pub fn _wcslwr() -> u64 {
    0
}

pub fn _wcslwr_s() -> u64 {
    0
}

pub fn _wcsnicmp() -> u64 {
    0
}

pub fn _wcsnset() -> u64 {
    0
}

pub fn _wcsnset_s() -> u64 {
    0
}

pub fn _wcsrev() -> u64 {
    0
}

pub fn _wcsset_s() -> u64 {
    0
}

pub fn _wcsupr() -> u64 {
    0
}

pub fn _wmakepath_s() -> u64 {
    0
}

pub fn _wsplitpath_s() -> u64 {
    0
}

pub fn _wtoi() -> u64 {
    0
}

pub fn _wtol() -> u64 {
    0
}

pub fn atoi() -> u64 {
    0
}

pub fn atol() -> u64 {
    0
}

pub fn bsearch() -> u64 {
    0
}

pub fn bsearch_s() -> u64 {
    0
}

pub fn isdigit() -> u64 {
    0
}

pub fn islower() -> u64 {
    0
}

pub fn isprint() -> u64 {
    0
}

pub fn isspace() -> u64 {
    0
}

pub fn isupper() -> u64 {
    0
}

pub fn iswalnum() -> u64 {
    0
}

pub fn iswdigit() -> u64 {
    0
}

pub fn iswspace() -> u64 {
    0
}

pub fn isxdigit() -> u64 {
    0
}

pub fn longjmp() -> u64 {
    0
}

pub fn mbstowcs() -> u64 {
    0
}

pub fn mbtowc() -> u64 {
    0
}

pub fn memchr() -> u64 {
    0
}

pub fn memcmp() -> u64 {
    0
}

pub fn memcpy() -> u64 {
    0
}

pub fn memcpy_s() -> u64 {
    0
}

pub fn memmove() -> u64 {
    0
}

pub fn memmove_s() -> u64 {
    0
}

pub fn memset() -> u64 {
    0
}

pub fn qsort() -> u64 {
    0
}

pub fn qsort_s() -> u64 {
    0
}

pub fn rand() -> u64 {
    0
}

pub fn sqrt() -> u64 {
    0
}

pub fn srand() -> u64 {
    0
}

pub fn strcat() -> u64 {
    0
}

pub fn strcat_s() -> u64 {
    0
}

pub fn strchr() -> u64 {
    0
}

pub fn strcmp() -> u64 {
    0
}

pub fn strcpy() -> u64 {
    0
}

pub fn strcpy_s() -> u64 {
    0
}

pub fn strlen() -> u64 {
    0
}

pub fn strncat() -> u64 {
    0
}

pub fn strncat_s() -> u64 {
    0
}

pub fn strncmp() -> u64 {
    0
}

pub fn strncpy() -> u64 {
    0
}

pub fn strncpy_s() -> u64 {
    0
}

pub fn strnlen() -> u64 {
    0
}

pub fn strrchr() -> u64 {
    0
}

pub fn strspn() -> u64 {
    0
}

pub fn strstr() -> u64 {
    0
}

pub fn strtok_s() -> u64 {
    0
}

pub fn tolower() -> u64 {
    0
}

pub fn toupper() -> u64 {
    0
}

pub fn towlower() -> u64 {
    0
}

pub fn towupper() -> u64 {
    0
}

pub fn vDbgPrintEx() -> u64 {
    0
}

pub fn vDbgPrintExWithPrefix() -> u64 {
    0
}

pub fn vsprintf() -> u64 {
    0
}

pub fn vsprintf_s() -> u64 {
    0
}

pub fn vswprintf_s() -> u64 {
    0
}

pub fn wcscat() -> u64 {
    0
}

pub fn wcscat_s() -> u64 {
    0
}

pub fn wcschr() -> u64 {
    0
}

pub fn wcscmp() -> u64 {
    0
}

pub fn wcscpy() -> u64 {
    0
}

pub fn wcscpy_s() -> u64 {
    0
}

pub fn wcscspn() -> u64 {
    0
}

pub fn wcslen() -> u64 {
    0
}

pub fn wcsncat() -> u64 {
    0
}

pub fn wcsncat_s() -> u64 {
    0
}

pub fn wcsncmp() -> u64 {
    0
}

pub fn wcsncpy() -> u64 {
    0
}

pub fn wcsncpy_s() -> u64 {
    0
}

pub fn wcsnlen() -> u64 {
    0
}

pub fn wcsrchr() -> u64 {
    0
}

pub fn wcsspn() -> u64 {
    0
}

pub fn wcsstr() -> u64 {
    0
}

pub fn wcstombs() -> u64 {
    0
}

pub fn wcstoul() -> u64 {
    0
}

pub fn wctomb() -> u64 {
    0
}

pub fn wine_enumerate_root_devices() -> u64 {
    0
}

pub fn wine_ntoskrnl_main_loop() -> u64 {
    0
}
