# Project manifest format

Big Star Editor projects are opened by selecting the project directory. The manifest is `big-star-project.yaml` inside that directory.

The manifest intentionally avoids absolute paths. Paths are stored relative to the project directory so projects can move between machines and operating systems. The project directory itself is not serialized because it is implied by the manifest location.

```yaml
format: big-star-editor-project
version: 1
name: "Example Project"
rom:
  extracted_path: "extracted-rom"
  game_title: "NEW MARIO"
  game_code: "A2DE"
files:
  - path: "extracted-rom/root/course/course1.bin"
    size: 1234
```

The backend owns a deliberately small YAML subset without a third-party YAML dependency. `format`, `version: 1`, and a nonempty `name` are required. Quoted strings support `\\`, `\"`, `\n`, `\r`, and `\t` escapes.

`rom.extracted_path` and every indexed file path must remain inside the project directory. Absolute paths, parent traversal, Windows drive prefixes, backslashes, duplicate file entries, invalid sizes, and files outside the configured extracted-ROM root are rejected. Manifest writes use an adjacent temporary file followed by atomic replacement, so a failed write does not leave a partially written manifest.
