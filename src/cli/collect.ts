import fs from "node:fs/promises";
import path from "node:path";
import { isSupportedExtension } from "../engine/index.js";

/**
 * Collects all supported image files under `dir`.
 *
 * @param dir        Directory to search.
 * @param recursive  Whether to descend into subdirectories.
 */
export async function collectFiles(dir: string, recursive: boolean): Promise<string[]> {
    const absDir = path.resolve(dir);

    let stat: Awaited<ReturnType<typeof fs.stat>>;
    try {
        stat = await fs.stat(absDir);
    } catch {
        throw new Error(`Cannot access path: ${absDir}`);
    }

    if (!stat.isDirectory()) {
        throw new Error(`Not a directory: ${absDir}`);
    }

    return walk(absDir, recursive);
}

async function walk(dir: string, recursive: boolean): Promise<string[]> {
    const entries = await fs.readdir(dir, { withFileTypes: true });
    const files: string[] = [];

    for (const entry of entries) {
        const fullPath = path.join(dir, entry.name);

        if (entry.isFile()) {
            const ext = path.extname(entry.name);
            if (isSupportedExtension(ext)) files.push(fullPath);
        } else if (recursive && entry.isDirectory()) {
            files.push(...(await walk(fullPath, true)));
        }
    }

    return files;
}

/**
 * Validates that a given path points to an existing supported image file.
 */
export async function validateFile(filePath: string): Promise<string | null> {
    try {
        const absPath = path.resolve(filePath);
        const stat = await fs.stat(absPath);
        if (!stat.isFile()) return null;

        const ext = path.extname(absPath);
        return isSupportedExtension(ext) ? absPath : null;
    } catch {
        return null;
    }
}
