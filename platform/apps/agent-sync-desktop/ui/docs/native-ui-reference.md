# Native UI Reference

This document maps official desktop design guidance to concrete Agent Sync UI rules.

## Sources

- Apple HIG (macOS): https://developer.apple.com/design/human-interface-guidelines/designing-for-macos
- Windows app design guidance: https://learn.microsoft.com/en-us/windows/apps/design/
- Fluent 2: https://fluent2.microsoft.design/
- GNOME HIG: https://developer.gnome.org/hig/

## Screenshot References

- Apple HIG visuals: https://developer.apple.com/design/human-interface-guidelines/designing-for-macos
- WinUI Gallery visuals: https://github.com/microsoft/WinUI-Gallery
- GNOME HIG patterns (header bar, controls): https://developer.gnome.org/hig/patterns/

## Rule Mapping

| Guideline rule                                              | Agent Sync implementation                                                       |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Use system typography and control metrics                   | `src/index.css` platform font variables and control height variables            |
| Respect system light/dark modes                             | `src/index.css` with `prefers-color-scheme` token overrides                     |
| Keep controls compact and readable for desktop density      | `src/components/ui/button.tsx`, `src/components/ui/input.tsx`                   |
| Preserve clear keyboard focus                               | shared focus ring tokens and component focus-visible classes                    |
| Support high-contrast / accessibility modes                 | `src/index.css` with `forced-colors` and contrast overrides                     |
| Use restrained visual treatment (avoid web-heavy gradients) | simplified background and muted card shadows in `src/index.css` and card styles |
| Platform-specific feel without forking logic                | `src/lib/platform.ts` sets `data-platform` attributes used by CSS               |

## UI Review Checklist

- System theme switch updates the app palette without restart.
- Controls (buttons, inputs, badges) keep native-feel size and contrast on all platforms.
- Focus state is visible and consistent for keyboard navigation.
- `forced-colors: active` remains usable on Windows high-contrast mode.
- Linux degrades safely when desktop environment is unknown.
