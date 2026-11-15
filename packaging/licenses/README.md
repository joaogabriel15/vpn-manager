The egui MIT and Apache license texts are from the upstream 0.33.3 tag:

- https://github.com/emilk/egui/blob/0.33.3/LICENSE-MIT
- https://github.com/emilk/egui/blob/0.33.3/LICENSE-APACHE

Several egui crates refer to these workspace-level licenses but do not include the
texts in their crate archives. They are included here for binary distributions.
The release packaging script also collects crate-local license files and font
licenses, preserving subdirectories. VPN Manager itself is MIT-licensed (see
LICENSE at the archive root); this folder covers dependencies only and does not
redistribute VPN engines.
