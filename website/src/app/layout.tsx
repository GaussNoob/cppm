import type { Metadata, Viewport } from "next";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";
import "./globals.css";
import "./responsive.css";
import "./refinement.css";
import "./header.css";
import "./documentation.css";

export const metadata: Metadata = {
  metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL ?? "https://cppm.gaussnoob.com.br"),
  title: "cppm — Menos setup. Mais C++.",
  description: "Do primeiro arquivo ao executável. Crie projetos C++, gerencie dependências com Conan 2 e compile com CMake e Ninja. Documentação e download do cppm.",
  applicationName: "cppm",
  openGraph: {
    title: "cppm — Menos setup. Mais C++.",
    description: "Seu próximo projeto C++ começa no terminal. Conheça o cppm.",
    locale: "pt_BR",
    type: "website",
    images: [{ url: "/og.png", width: 1200, height: 630, alt: "cppm — Menos setup. Mais C++." }],
  },
  twitter: { card: "summary_large_image" },
};

export const viewport: Viewport = { themeColor: "#111410", colorScheme: "dark" };

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="pt-BR" className={`${GeistSans.variable} ${GeistMono.variable}`}>
      <body>{children}</body>
    </html>
  );
}
