---
id: tools
title: Move Tools
custom_edit_url: https://github.com/move-language/move/edit/main/language/tools/README.md
---

# Summary-English

Move has a number of tools associated with it. This directory contains all,
or almost all of them. The following crates in this directory are libraries
that are used by the [`move-cli`](./move-cli) `package` subcommand:

* `move-bytecode-viewer`
* `move-disassembler`
* `move-explain`
* `move-unit-test`
* `move-package`
* `move-coverage`

In this sense each of these crates defines the core logic for a specific
package command, e.g., how to run and report unit tests, or collect and
display test coverage information. However, the Move CLI is responsible for
stitching these commands together, e.g., when running a move unit test the
Move CLI is responsible for first making sure the package was built in
`test` mode ( using the `move-package` library), collecting the test plan
to feed to the `move-unit-test` library, and returning a non-zero error
code if a test fails.

Generally, if you want to see how various tools interact with each other,
or how a normal Move user would interact with these tools, you should first
look at the Move CLI (specifically the `package` subdirectory/command) as
that is responsible for stitching everything together. If you are looking
for where the logic for a specific tool is defined, this is most likely in
the specific crate for that tool (e.g., if you want to see how TUIs are
handled for the `move-bytecode-viewer` that's defined in the
`move-bytecode-viewer` crate, and not the `move-cli` crate).

Some of the crates mentioned above are also binaries at the moment, however
they should all be able to be made libaries only, with the possible
exception of the `move-coverage` crate. The primary reason for this, is
that this tool can collect and report test coverage statistics across
multiple packages, and multiple runs over a package. This functionality is
important if you have a large functional test suite such as Diem's and want
to gather coverage information across all of them.

The `move-resource-viewer`, and `read-write-set` similarly are library
crates that are used by and exposed by the Move CLI, but not through the
`package` subcommand.

The `move-bytecode-utils` crates holds general
utilities for working with Move bytecode, e.g., computing the dependency
order for modules.


# 概述-中文

Move 具有多个相关的工具。这个目录包含了所有或几乎所有的工具。目录中的以下 crate 是由 [`move-cli`](./move-cli) `package` 子命令使用的库：

* `move-bytecode-viewer`
* `move-disassembler`
* `move-explain`
* `move-unit-test`
* `move-package`
* `move-coverage`

在这个意义上，这些 crate 定义了特定包命令的核心逻辑，例如，如何运行和报告单元测试，或如何收集和显示测试覆盖信息。然而，Move CLI 负责将这些命令串联在一起。例如，当运行 Move 单元测试时，Move CLI 需要首先确保包以 `test` 模式构建（使用 `move-package` 库），收集测试计划并传递给 `move-unit-test` 库，如果测试失败则返回非零错误代码。

通常，如果你想了解各种工具如何相互交互，或者一个普通的 Move 用户如何与这些工具交互，你应该首先查看 Move CLI（特别是 `package` 子目录/命令），因为它负责将所有内容连接在一起。如果你在寻找某个特定工具的逻辑定义，最可能的情况是它在该工具的具体 crate 中定义（例如，如果你想了解 `move-bytecode-viewer` 如何处理 TUI，那是在 `move-bytecode-viewer` crate 中定义的，而不是 `move-cli` crate）。

上述提到的某些 crate 目前也作为二进制文件存在，但它们都应该能够仅作为库来使用，可能唯一的例外是 `move-coverage` crate。之所以这样，是因为该工具能够跨多个包收集和报告测试覆盖率统计信息，并且可以对同一包进行多次运行。这项功能在你有一个庞大的功能测试套件（例如 Diem 的测试套件）并希望收集所有测试的覆盖信息时非常重要。

`move-resource-viewer` 和 `read-write-set` 类似，都是库 crate，供 Move CLI 使用并通过 Move CLI 暴露，但不是通过 `package` 子命令。

`move-bytecode-utils` crate 提供了处理 Move 字节码的通用工具，例如计算模块的依赖顺序。
