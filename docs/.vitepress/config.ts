import { defineConfig } from 'vitepress'

const zhNav = [
  { text: '指南', link: '/guide/getting-started' },
  { text: 'CLI', link: '/cli/overview' },
  { text: 'Skills', link: '/skills/overview' },
]

const enNav = [
  { text: 'Guide', link: '/en/guide/getting-started' },
  { text: 'CLI', link: '/en/cli/overview' },
  { text: 'Skills', link: '/en/skills/overview' },
]

const zhSidebar = [
  {
    text: '指南',
    items: [{ text: '快速开始', link: '/guide/getting-started' }],
  },
  {
    text: 'CLI',
    items: [
      { text: '总览', link: '/cli/overview' },
      { text: 'library', link: '/cli/library' },
      { text: 'item', link: '/cli/item' },
      { text: 'collection', link: '/cli/collection' },
      { text: 'workspace', link: '/cli/workspace' },
      { text: 'sync / mcp', link: '/cli/sync-mcp' },
      { text: '故障排查', link: '/cli/troubleshooting' },
    ],
  },
  {
    text: 'Skills',
    items: [
      { text: '总览', link: '/skills/overview' },
      { text: '路由策略', link: '/skills/routing' },
      { text: '安全边界', link: '/skills/safety' },
      { text: '典型工作流', link: '/skills/workflows' },
      { text: 'Fallback', link: '/skills/fallback' },
    ],
  },
]

const enSidebar = [
  {
    text: 'Guide',
    items: [{ text: 'Getting Started', link: '/en/guide/getting-started' }],
  },
  {
    text: 'CLI',
    items: [
      { text: 'Overview', link: '/en/cli/overview' },
      { text: 'library', link: '/en/cli/library' },
      { text: 'item', link: '/en/cli/item' },
      { text: 'collection', link: '/en/cli/collection' },
      { text: 'workspace', link: '/en/cli/workspace' },
      { text: 'sync / mcp', link: '/en/cli/sync-mcp' },
      { text: 'Troubleshooting', link: '/en/cli/troubleshooting' },
    ],
  },
  {
    text: 'Skills',
    items: [
      { text: 'Overview', link: '/en/skills/overview' },
      { text: 'Routing', link: '/en/skills/routing' },
      { text: 'Safety', link: '/en/skills/safety' },
      { text: 'Workflows', link: '/en/skills/workflows' },
      { text: 'Fallbacks', link: '/en/skills/fallback' },
    ],
  },
]

export default defineConfig({
  title: 'zot',
  description: 'Rust Zotero CLI and zot-skills documentation',
  cleanUrls: true,
  lastUpdated: true,
  themeConfig: {
    search: {
      provider: 'local',
    },
  },
  locales: {
    root: {
      lang: 'zh-CN',
      label: '简体中文',
      title: 'zot 文档',
      description: 'Rust Zotero CLI 与 zot-skills 使用文档',
      themeConfig: {
        nav: zhNav,
        sidebar: zhSidebar,
        outline: [2, 3],
        docFooter: {
          prev: '上一页',
          next: '下一页',
        },
        lastUpdatedText: '最后更新',
      },
    },
    en: {
      lang: 'en-US',
      label: 'English',
      link: '/en/',
      title: 'zot Docs',
      description: 'Documentation for the Rust Zotero CLI and zot-skills',
      themeConfig: {
        nav: enNav,
        sidebar: enSidebar,
        outline: [2, 3],
        docFooter: {
          prev: 'Previous page',
          next: 'Next page',
        },
        lastUpdatedText: 'Last updated',
      },
    },
  },
})
