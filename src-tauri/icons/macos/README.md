# macOS app icon

The asset catalog contains the same Atogaki illustration as the legacy ICNS, without pre-cut corners: current macOS applies the icon mask. Tauri bundles the compiled catalog with `CFBundleIconName=AppIcon` and retains `CFBundleIconFile=icon.icns` for older systems. To refresh the checked-in catalog from this directory with Xcode:

```sh
xcrun actool Assets.xcassets --compile . --platform macosx --minimum-deployment-target 12.0 --app-icon AppIcon --output-partial-info-plist /tmp/atogaki-app-icon.plist
```

Use the resulting `Assets.car`; do not replace the legacy `../icon.icns` with the single-rendition ICNS emitted by this command.
