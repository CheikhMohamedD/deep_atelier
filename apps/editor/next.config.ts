import type { NextConfig } from "next";
import createNextIntlPlugin from "next-intl/plugin";

const withNextIntl = createNextIntlPlugin("./i18n/request.ts");

const config: NextConfig = {
  reactStrictMode: true,
  // Paquets du workspace livrés en TypeScript source.
  transpilePackages: [
    "@deep-atelier/engine",
    "@deep-atelier/ir-types",
    "@deep-atelier/canvas-protocol",
  ],
};

export default withNextIntl(config);
