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

M0/M1 uses a deliberately small YAML subset written by the backend without adding a third-party YAML dependency. If future project metadata becomes richer, add a documented dependency decision before replacing the parser.
