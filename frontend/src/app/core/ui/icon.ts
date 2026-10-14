import { ChangeDetectionStrategy, Component, computed, input } from "@angular/core";

// Íconos de trazo 24px, trazo 1.75, color = currentColor (compatibles con Lucide).
const ICONS: Record<string, string[]> = {
  "alert": ["M12 9v4", "M12 17h.01", "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"],
  "arrow-down": ["M7 7l10 10", "M17 8v9H8"],
  "arrow-up": ["M7 17L17 7", "M8 7h9v9"],
  "bell": ["M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9", "M13.73 21a2 2 0 0 1-3.46 0"],
  "check": ["M20 6L9 17l-5-5"],
  "chevron-down": ["M6 9l6 6 6-6"],
  "chevron-right": ["M9 6l6 6-6 6"],
  "clients": ["M3 21h18", "M5 21V7l7-4 7 4v14", "M9 9h1", "M14 9h1", "M9 13h1", "M14 13h1", "M10 21v-4h4v4"],
  "clock": ["M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z", "M12 6v6l4 2"],
  "copy": ["M9 9h13v13H9z", "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"],
  "dashboard": ["M3 3h7v9H3z", "M14 3h7v5h-7z", "M14 12h7v9h-7z", "M3 16h7v5H3z"],
  "download": ["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M7 10l5 5 5-5", "M12 15V3"],
  "edit": ["M12 20h9", "M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z"],
  "expenses": ["M3 7h16a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z", "M3 7l13-4v4", "M16 13.5h2"],
  "eye-off": ["M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94", "M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19", "M1 1l22 22"],
  "eye": ["M1 12s4-8 11-8 11 8 11 8-4 8-11 8S1 12 1 12z", "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"],
  "file": ["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z", "M14 2v6h6"],
  "filter": ["M22 3H2l8 9.46V19l4 2v-8.54z"],
  "invoices": ["M5 2h14v20l-3-2-2 2-2-2-2 2-2-2-3 2z", "M9 7h6", "M9 11h6", "M9 15h4"],
  "logout": ["M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4", "M16 17l5-5-5-5", "M21 12H9"],
  "mail": ["M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z", "M22 6l-10 7L2 6"],
  "orders": ["M21 8l-9-5-9 5 9 5 9-5z", "M3 8v8l9 5 9-5V8", "M12 13v8"],
  "plus": ["M12 5v14", "M5 12h14"],
  "quotes": ["M4 2h16v20H4z", "M8 6h8", "M8 10h2", "M14 10h2", "M8 14h2", "M14 14h2", "M8 18h2", "M14 18h2"],
  "refresh": ["M23 4v6h-6", "M1 20v-6h6", "M3.51 9a9 9 0 0 1 14.85-3.36L23 10", "M20.49 15a9 9 0 0 1-14.85 3.36L1 14"],
  "reports": ["M3 3v18h18", "M8 17V11", "M13 17V7", "M18 17v-4"],
  "search": ["M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16z", "M21 21l-4.35-4.35"],
  "settings": ["M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z", "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"],
  "sparkle": ["M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z", "M19 17l.8 2.2L22 20l-2.2.8L19 23l-.8-2.2L16 20l2.2-.8z"],
  "suppliers": ["M1 3h15v13H1z", "M16 8h4l3 3v5h-7z", "M5.5 18.5a2.5 2.5 0 1 0 0 .01", "M18.5 18.5a2.5 2.5 0 1 0 0 .01"],
  "team": ["M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2", "M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z", "M22 21v-2a4 4 0 0 0-3-3.87", "M16 3.13a4 4 0 0 1 0 7.75"],
  "tenders": ["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z", "M14 2v6h6", "M8 13h8", "M8 17h5"],
  "trash": ["M3 6h18", "M8 6V4h8v2", "M19 6l-1 14H6L5 6", "M10 11v6", "M14 11v6"],
  "upload": ["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M17 8l-5-5-5 5", "M12 3v12"],
  "x": ["M18 6L6 18", "M6 6l12 12"],
};
export type IconName = keyof typeof ICONS;

@Component({
  selector: "bs-icon",
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: { style: "display:contents" },
  template: `<svg class="bs-icon" viewBox="0 0 24 24" [attr.width]="size()" [attr.height]="size()"
    fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
    [attr.aria-hidden]="label() ? null : 'true'" [attr.aria-label]="label() || null" [attr.role]="label() ? 'img' : null">
    @for (d of paths(); track $index) { <path [attr.d]="d" /> }
  </svg>`,
})
export class Icon {
  name = input.required<string>();
  size = input(20);
  /** Solo si el ícono va sin texto visible al lado */
  label = input<string>("");
  protected paths = computed(() => ICONS[this.name()] ?? []);
}
