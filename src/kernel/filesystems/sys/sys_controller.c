#include <malloc.h>
#include <pci.h
  }

  return toCopy;
}

VfsHandlers handlePciConfig = {.read = pciConfigRead,
                               .write = 0,
                               .stat = fakefsFstat,
                               .duplicate = 0,
                               .ioctl = 0,
                               .mmap = 0,
                               .getdents64 = 0,
                               .seek = fsSimpleSeek};

void sysSetupPci(FakefsFile *devices) {
  PCIdevice        *device = (PCIdevice *)malloc(sizeof(PCIdevice));
  PCIgeneralDevice *out = (PCIgeneralDevice *)malloc(sizeof(PCIgeneralDevice));

  for (uint16_t bus = 0; bus < PCI_MAX_BUSES; bus++) {
    for (uint8_t slot = 0; slot < PCI_MAX_DEVICES; slot++) {
      for (uint8_t function = 0; function < PCI_MAX_FUNCTIONS; function++) {
        if (!FilterDevice(bus, slot, function))
          continue;

        GetDevice(device, bus, slot, function);
        GetGeneralDevice(device, out);

        char *dirname = (char *)malloc(128);
        sprintf(dirname, "0000:%02d:%02d.%d", bus, slot, function);

        PciConf *pciconf = (PciConf *)malloc(sizeof(PciConf));
        pciconf->bus = bus;
        pciconf->slot = slot;
        pciconf->function = function;

        FakefsFile *dir =
            fakefsAddFile(&rootSys, devices, dirname, 0,
                          S_IFDIR | S_IRUSR | S_IWUSR, &fakefsRootHandlers);

        // [..]/config
        FakefsFile *confFile =
            fakefsAddFile(&rootSys, dir, "config", 0,
                          S_IFREG | S_IRUSR | S_IWUSR, &handlePciConfig);
        fakefsAttachFile(confFile, pciconf, 4096);

        // [..]/vendor
        char *vendorStr = (char *)malloc(8);
        sprintf(vendorStr, "0x%04x\n", device->vendor_id);
        FakefsFile *vendorFile = fakefsAddFile(&rootSys, dir, "vendor", 0,
                                               S_IFREG | S_IRUSR | S_IWUSR,
                                               &fakefsSimpleReadHandlers);
        fakefsAttachFile(vendorFile, vendorStr, 4096);

        // [..]/irq
        char *irqStr = (char *)malloc(8);
        sprintf(irqStr, "%d\n", out->interruptLine);
        FakefsFile *irqFile =
            fakefsAddFile(&rootSys, dir, "irq", 0, S_IFREG | S_IRUSR | S_IWUSR,
                          &fakefsSimpleReadHandlers);
        fakefsAttachFile(irqFile, irqStr, 4096);

        // [..]/revision
        char *revisi
