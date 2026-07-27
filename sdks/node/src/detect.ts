import * as fs from 'fs';
import * as path from 'path';

export interface DetectionResult {
  service_name: string;
  language: string;
  framework: string | null;
  project_file: string | null;
}

export function detectFramework(workspace?: string): DetectionResult {
  const dir = workspace ?? process.cwd();

  const result: DetectionResult = {
    service_name: 'unknown-service',
    language: 'unknown',
    framework: null,
    project_file: null,
  };

  // Check package.json
  const pkgPath = path.join(dir, 'package.json');
  try {
    if (fs.existsSync(pkgPath)) {
      const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));
      result.language = 'node';
      result.project_file = 'package.json';
      if (pkg.name) result.service_name = pkg.name;

      const deps = { ...(pkg.dependencies ?? {}), ...(pkg.devDependencies ?? {}) };
      if (deps.next) result.framework = 'next';
      else if (deps.express) result.framework = 'express';
      else if (deps.fastify) result.framework = 'fastify';
      else if (deps.nest) result.framework = 'nestjs';
      else if (deps.koa) result.framework = 'koa';
    }
  } catch {
    // ignore
  }

  return result;
}

export function writeConfig(detection: DetectionResult, outPath?: string): void {
  const configPath = outPath ?? path.join(process.cwd(), 'greplog.config.json');
  const config = {
    service_name: detection.service_name,
    language: detection.language,
    framework: detection.framework,
    detected_at: new Date().toISOString(),
  };

  try {
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
  } catch {
    // fail-open: config write is best-effort
  }
}
