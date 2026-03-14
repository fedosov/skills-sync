# Native UI Reference

This document maps official desktop design guidance to concrete Dotagents Desktop UI rules.

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

| Guideline rule                                    | Dotagents Desktop implementation                                            |
| ------------------------------------------------- | --------------------------------------------------------------------------- |
| Use system typography and compact desktop spacing | `src/index.css` font stack, spacing, and control-size variables             |
| Keep primary actions obvious but restrained       | `src/components/ui/button.tsx` variants and `src/App.tsx` top-level actions |
| Preserve visible keyboard focus                   | shared `focus-visible` ring styles in button and input primitives           |
| Support high-contrast / accessibility modes       | `src/index.css` token choices and semantic color variables                  |
| Avoid web-heavy visual noise                      | quiet card treatment and neutral surfaces in `src/index.css`                |
| Keep command context legible                      | `src/App.tsx` runtime banner, scope summary, and output transcript layout   |

## UI Review Checklist

- The runtime banner clearly distinguishes bundled-runtime success from failure.
- Scope and project-root controls stay visible regardless of the active tab.
- Form controls remain compact and readable on desktop window sizes.
- The output transcript is scannable without hiding stdout or stderr details.
