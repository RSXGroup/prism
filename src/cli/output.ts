import path from "node:path";

/**
 * Derives the output path for a processed file.
 *
 * Strategy: suffix the stem before the extension.
 * e.g. `/assets/icon.png` → `/assets/icon.fixed.png`
 *
 * @param inputPath  Absolute path of the source file.
 * @param suffix     Suffix to append to the stem (default: `"fixed"`).
 */
export function resolveOutputPath(inputPath: string, suffix = "fixed"): string {
    const dir  = path.dirname(inputPath);
    const ext  = path.extname(inputPath);
    const stem = path.basename(inputPath, ext);

    return path.join(dir, `${stem}.${suffix}${ext}`);
}
