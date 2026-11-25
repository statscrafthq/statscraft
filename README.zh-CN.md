# Statscraft

[English](README.md) · [Español](README.es.md) · 简体中文

本译文对应 0.48.0 版的英文 README.md。两者不一致时，以英文为准。

Statscraft 是一款面向社会科学的免费开源统计软件，从第一行代码起就为 AI 智能体而设计。它可以替代 PSPP、Stata 和 SAS：读取 .sav 文件，运行您现有的语法，支持 Windows、macOS 和 Linux。两个用 Rust 编写的程序共用一个引擎：命令行程序 `statscraft` 和桌面窗口程序 `statscraft-gui`。

[下载](https://statscraft.org/download.html) · [文档](https://docs.statscraft.org) · [网站](https://statscraft.org)

## 功能

- **数据文件。** 读写 .sav 和 .zsav 文件，保留标签、缺失值和多重响应集。可以打开 Excel 工作簿和文本文件，并写出 Excel、CSV 和 Stata 文件。
- **数据准备。** 计算、重新编码、筛选、加权、汇总、合并和重构数据，支持宏和矩阵语言。
- **自定义表格。** 按组统计计数、百分比和均值，支持总计、小计、多重响应集，以及用字母标注的显著性检验。
- **统计分析。** 频数、交叉表、均值、t 检验和方差分析；线性回归、逻辑回归及其他多种回归；一般线性模型、广义线性模型和混合线性模型；因子分析、聚类分析和决策树分析；生存分析、预测、复杂抽样、贝叶斯检验和功效分析。
- **输出。** 表格和图表可导出到 Excel、Word、PowerPoint、PDF 和网页。图表在办公软件中仍是可编辑的图表。
- **AI 智能体。** 提供 MCP 服务器、JSON 输出、在多次运行之间保留数据的会话，以及标明行号和列号的错误信息，让智能体完成分析，您在窗口中查看过程。

[命令页面](docs/commands/README.md)（英文）列出了全部命令。

Statscraft 是一个独立项目，与其他统计软件的开发商及 ArtCraft 套件均无关联，也未获得其认可。产品名称是其各自所有者的商标。

状态：早期开发阶段。采用 MIT 许可证。

## 文档

文档位于 [docs.statscraft.org](https://docs.statscraft.org)，同样的页面也在 `docs/` 目录中。目前为英文。

- [入门](docs/getting-started.md)：从安装到在 Excel 中生成一张自定义表格，附示例文件。
- [安装与更新](docs/install.md)：各个安装包、卸载和更新。
- [窗口](docs/window.md)：数据编辑器、语法编辑器、输出、菜单和自定义表格。
- [命令行](docs/command-line.md)：所有子命令和选项、会话、JSON 输出、退出代码、导出到 Excel 以及窗口的实时会话。
- [数据库](docs/databases.md)：将数据集存入 SQLite、PostgreSQL、MySQL、MariaDB 或 Dolt，并在数据库中重新编码。
- [AI 智能体](docs/ai-agents.md)：智能体应如何使用 Statscraft，以及可粘贴到您的智能体中的说明。
- [连接您的 AI 智能体](docs/connect.md)：将 Statscraft 添加到您电脑上的 AI 智能体。
- [MCP 服务器](docs/mcp.md)：智能体应用获得的工具、会话和安全性。
- [插件](docs/plugins.md)：为自定义表格的单元格着色和添加注释的程序。
- [命令](docs/commands/README.md)：每个命令一页。
- [更新日志](CHANGELOG.md)：每个版本的内容。

## 参与贡献

[CONTRIBUTING.md](CONTRIBUTING.md) 说明如何从源代码构建、运行测试和发布版本。[开发者说明](docs/developers/README.md)和[编写插件](docs/developers/plugins.md)提供了用户指南背后的细节。
