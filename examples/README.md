# Examples

## sample.diff

A realistic multi-file diff showing authentication improvements and route changes in a Rust web application. Two files are modified:

- `src/auth.rs` — Token validation upgraded from a simple length check to a proper result-based validation with config support
- `src/api/routes.rs` — Rate limiting added to API routes, new refresh token endpoint

## demo.sh

Shell script that generates walkthrough videos:

1. From the bundled `sample.diff` file
2. From the most recent git commit (if run inside a git repository)

### Usage

```bash
# From the patchcast project root
chmod +x examples/demo.sh
./examples/demo.sh
```

Output videos are written to `examples/output/`.

## Output

The `output/` directory is gitignored and used for generated video files.
