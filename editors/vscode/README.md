# CodeGrid for Visual Studio Code

[English](#english) · [Deutsch](#deutsch) · [Français](#français) · [Español (España)](#español-españa) · [繁體中文](#繁體中文) · [日本語](#日本語) · [简体中文](#简体中文) · [한국어](#한국어) · [Português (Brasil)](#português-brasil) · [Русский](#русский)

## Deutsch

CodeGrid für Visual Studio Code unterstützt die vollständige CodeGrid-Sprache in `.cg`-Dateien: Syntaxhervorhebung, Formatierung, Vervollständigung, Vorlagen und Hilfe beim Überfahren mit der Maus. Diese Editorfunktionen benötigen weder Rust noch einen Sprachserver.

Öffnen und speichern Sie eine `.cg`-Datei. Über die Schaltflächen im Editor können Sie sie ausführen oder debuggen. **F5** startet das Debuggen, **Strg+F5** die Ausführung. Unterstützt werden Haltepunkte, Pause, schrittweise Ausführung, Aufrufstapel, Register, Speicher und schreibgeschützte Überwachungsausdrücke. Jeder Schritt führt einen atomaren Global Tick für alle aktiven Threads aus; Custom lässt sich innerhalb dieses Ticks nicht einzeln anhalten.

Das Windows-x64-Paket enthält die native Laufzeit. Andere Plattformen können `codegrid.runtime.path` konfigurieren. Die Ausführung erfordert einen vertrauenswürdigen Arbeitsbereich. Die Oberfläche folgt der VS-Code-Anzeigesprache; Änderungen erfordern einen Neustart. Fehler werden mit stabilen Codes und übersetzten Meldungen angezeigt; Originaldiagnosen bleiben in den Details verfügbar. Die Sprachsyntax bleibt unverändert. Die ausführliche Referenz folgt im englischen Abschnitt.

## Français

CodeGrid pour Visual Studio Code prend en charge le langage CodeGrid complet dans les fichiers `.cg` : coloration syntaxique, formatage, complétion, modèles et aide au survol. Ces fonctions d’édition ne nécessitent ni Rust ni serveur de langage.

Ouvrez et enregistrez un fichier `.cg`, puis utilisez les boutons d’exécution ou de débogage dans l’éditeur. **F5** lance le débogage et **Ctrl+F5** l’exécution. Vous disposez de points d’arrêt, de la pause, du pas à pas, de la pile d’appels, des registres, de la mémoire et d’expressions de surveillance en lecture seule. Chaque pas exécute un Global Tick atomique pour tous les fils actifs ; Custom ne peut pas être suspendu séparément à l’intérieur de ce tick.

Le paquet Windows x64 inclut le moteur natif. Sur les autres plateformes, configurez `codegrid.runtime.path`. L’exécution exige un espace de travail approuvé. L’interface suit la langue d’affichage de VS Code après redémarrage. Les erreurs affichent des codes stables et des messages traduits ; les diagnostics originaux restent disponibles dans les détails. La syntaxe reste inchangée. La référence détaillée figure dans la section anglaise.

## Español (España)

CodeGrid para Visual Studio Code admite el lenguaje CodeGrid completo en archivos `.cg`: resaltado de sintaxis, formato, autocompletado, plantillas y ayuda al pasar el ratón. Estas funciones de edición no requieren Rust ni un servidor de lenguaje.

Abra y guarde un archivo `.cg` y utilice los botones de ejecución o depuración del editor. **F5** inicia la depuración y **Ctrl+F5** ejecuta el programa. Se admiten puntos de interrupción, pausa, ejecución paso a paso, pila de llamadas, registros, memoria y expresiones de inspección de solo lectura. Cada paso ejecuta un Global Tick atómico para todos los hilos activos; Custom no puede detenerse de forma independiente dentro de ese tick.

El paquete para Windows x64 incluye el motor nativo. En otras plataformas, configure `codegrid.runtime.path`. La ejecución requiere un espacio de trabajo de confianza. La interfaz sigue el idioma de VS Code tras reiniciarlo. Los errores muestran códigos estables y mensajes traducidos; los diagnósticos originales siguen disponibles en los detalles. La sintaxis no cambia. La referencia detallada está en la sección en inglés.

## 繁體中文

CodeGrid 擴充套件支援 `.cg` 檔案的完整 CodeGrid 語言，提供語法醒目提示、格式化、自動完成、範本及游標懸停說明。這些編輯功能不需要安裝 Rust 或語言伺服器。

開啟並儲存 `.cg` 檔案，即可透過編輯器上的按鈕執行或偵錯。**F5** 啟動偵錯，**Ctrl+F5** 直接執行。支援中斷點、暫停、逐步執行、呼叫堆疊，以及暫存器、記憶體和唯讀監看運算式。每次步進對所有活動執行緒執行一個原子 Global Tick；Custom 在此 Tick 內完成，無法獨立暫停內部指令。

Windows x64 安裝包內建原生執行環境；其他平台可設定 `codegrid.runtime.path`。執行前須信任工作區。介面會跟隨 VS Code 的顯示語言，切換後請重新啟動。錯誤會顯示穩定錯誤碼與翻譯訊息；原始診斷仍可在詳細資訊中查看，語法指令維持不變。詳細參考文件請見英語章節。

## 日本語

CodeGrid の Visual Studio Code 拡張機能は、`.cg` ファイルの完全な CodeGrid 言語に対応し、構文の強調表示、フォーマット、補完、テンプレート、ホバーによる説明を提供します。これらの編集機能に Rust や言語サーバーは不要です。

`.cg` ファイルを開いて保存し、エディターの実行またはデバッグボタンを使用してください。**F5** でデバッグ、**Ctrl+F5** で実行できます。ブレークポイント、一時停止、ステップ実行、コールスタック、レジスタ、メモリ、読み取り専用のウォッチ式に対応しています。各ステップは全ての実行中スレッドに対して 1 つのアトミックな Global Tick を実行します。Custom はこの Tick 内で完了し、内部命令だけを個別に停止することはできません。

Windows x64 パッケージにはネイティブランタイムが同梱されています。他のプラットフォームでは `codegrid.runtime.path` を設定できます。実行にはワークスペースの信頼が必要です。表示言語は VS Code に従い、変更後は再起動してください。エラーには安定したコードと翻訳されたメッセージが表示され、元の診断は詳細で確認できます。構文は変わりません。詳細は英語のリファレンスを参照してください。

## 简体中文

CodeGrid 扩展支持 `.cg` 文件的完整 CodeGrid 语言，提供语法高亮、格式化、自动补全、模板及悬停说明。这些编辑功能不需要安装 Rust 或语言服务器。

打开并保存 `.cg` 文件，即可通过编辑器上的按钮运行或调试。**F5** 启动调试，**Ctrl+F5** 直接运行。支持断点、暂停、单步执行、调用栈，以及寄存器、内存和只读监视表达式。每次单步对所有活动线程执行一个原子 Global Tick；Custom 在该 Tick 内完成，无法独立暂停内部指令。

Windows x64 安装包内置原生运行环境；其他平台可设置 `codegrid.runtime.path`。运行前须信任工作区。界面跟随 VS Code 的显示语言，切换后请重新启动。错误会显示稳定错误码与翻译消息；原始诊断仍可在详细信息中查看，语法指令保持不变。详细参考文档请见英语章节。

## 한국어

Visual Studio Code용 CodeGrid 확장은 `.cg` 파일의 전체 CodeGrid 언어를 지원하며 구문 강조, 서식 지정, 자동 완성, 템플릿 및 마우스 오버 설명을 제공합니다. 이러한 편집 기능에는 Rust나 언어 서버가 필요하지 않습니다.

`.cg` 파일을 열고 저장한 후 편집기의 실행 또는 디버그 버튼을 사용하세요. **F5**는 디버깅을, **Ctrl+F5**는 실행을 시작합니다. 중단점, 일시 중지, 단계 실행, 호출 스택, 레지스터, 메모리 및 읽기 전용 조사식을 지원합니다. 각 단계는 모든 활성 스레드에서 하나의 원자적 Global Tick을 실행합니다. Custom은 이 Tick 안에서 완료되므로 내부 명령만 따로 일시 중지할 수 없습니다.

Windows x64 패키지에는 네이티브 런타임이 포함됩니다. 다른 플랫폼에서는 `codegrid.runtime.path`를 설정할 수 있습니다. 실행하려면 작업 영역을 신뢰해야 합니다. 인터페이스는 VS Code 표시 언어를 따르며 변경 후 다시 시작해야 합니다. 오류에는 안정적인 코드와 번역된 메시지가 표시되며 원본 진단은 자세한 정보에서 확인할 수 있습니다. 구문은 바뀌지 않습니다. 자세한 내용은 영어 참조 문서를 확인하세요.

## Português (Brasil)

A extensão CodeGrid para Visual Studio Code oferece suporte à linguagem CodeGrid completa em arquivos `.cg`, com destaque de sintaxe, formatação, preenchimento automático, modelos e ajuda ao passar o mouse. Essas funções de edição não exigem Rust nem servidor de linguagem.

Abra e salve um arquivo `.cg` e use os botões de execução ou depuração do editor. **F5** inicia a depuração e **Ctrl+F5** executa o programa. Há suporte a pontos de interrupção, pausa, execução passo a passo, pilha de chamadas, registradores, memória e expressões de inspeção somente leitura. Cada passo executa um Global Tick atômico em todas as threads ativas; Custom termina dentro desse tick e suas instruções internas não podem ser pausadas separadamente.

O pacote Windows x64 inclui o runtime nativo. Em outras plataformas, configure `codegrid.runtime.path`. A execução requer um espaço de trabalho confiável. A interface segue o idioma de exibição do VS Code após reiniciar. Os erros exibem códigos estáveis e mensagens traduzidas; os diagnósticos originais continuam disponíveis nos detalhes. A sintaxe não muda. A referência detalhada está na seção em inglês.

## Русский

Расширение CodeGrid для Visual Studio Code поддерживает полный язык CodeGrid в файлах `.cg`: подсветку синтаксиса, форматирование, автодополнение, шаблоны и подсказки при наведении. Эти функции редактора не требуют Rust или языкового сервера.

Откройте и сохраните файл `.cg`, затем используйте кнопки запуска или отладки в редакторе. **F5** запускает отладку, **Ctrl+F5** — выполнение. Поддерживаются точки останова, пауза, пошаговое выполнение, стек вызовов, регистры, память и выражения наблюдения только для чтения. Каждый шаг выполняет один атомарный Global Tick для всех активных потоков; Custom завершается внутри этого Tick, поэтому его внутренние инструкции нельзя приостанавливать отдельно.

Пакет Windows x64 включает встроенную среду выполнения. На других платформах настройте `codegrid.runtime.path`. Для запуска рабочая область должна быть доверенной. Интерфейс следует языку VS Code после перезапуска. Ошибки показывают стабильные коды и переведённые сообщения; исходная диагностика доступна в подробностях. Синтаксис не меняется. Подробная справка приведена в английском разделе.

## English

Standalone syntax highlighting, conservative formatting, and basic editor
assistance for the Full CodeGrid source language in `.cg` files. These editor
features work without Rust, the CodeGrid CLI, or a language server.

### Interface languages

The extension follows the VS Code display language. Supported languages are
English, German, French, Spanish (Spain), Traditional Chinese, Japanese,
Simplified Chinese, Korean, Portuguese (Brazil), and Russian. Select a language
with VS Code's **Configure Display Language** command and restart VS Code.
Unsupported display languages fall back to English.

Commands, settings, templates, status messages, standalone completion and hover
help, editor-owned execution messages, and debugger scopes are localized.
CodeGrid instruction spellings, instruction names, error identifiers, watch
paths, and serialized VM state remain stable. Source diagnostics, native debug
failures, VM exceptions and editor errors display stable identities and localized
messages. Original diagnostics remain available as related information; native
exception details remain available in the debugger. LSP-provided instruction
help and operating-system details retain their original text. The language core
does not depend on editor locale.

### Run and Debug

Open a `.cg` file and use the Run or Debug button in the editor title bar,
the editor context menu, or `CodeGrid: Run Current File` / `CodeGrid: Debug
Current File` in the Command Palette. F5 starts debugging; Ctrl+F5 runs
without debugging. A launch configuration is optional for the current file.
The native runtime compiles the current in-memory document, including unsaved
edits. Untitled documents must first be saved. Restart a session after editing
its source; locations refer to the source captured when that session launched.

The Windows x64 installation package includes the native runtime. Other
platforms can set `codegrid.runtime.path` to a matching `codegrid` executable
built from this repository. An empty path selects the bundled executable and
then PATH. The language server is not required for execution. Execution
requires a trusted workspace.

- Run writes output bytes and termination information to the Debug Console.
- Debug stops on entry by default and supports continue, pause, stop, and
  single-step. Step Over and Step Into both advance one atomic Global Tick
  across every live Outer thread. Step Out continues until the selected
  thread returns to a shallower Function call stack, a breakpoint is reached,
  or the VM terminates or hits a configured limit.
- Set line breakpoints on grid rows. A line breakpoint binds to its first
  cell. Use an inline/column breakpoint to select another cell on that row.
  Main, Outer Function, and their Folded Block cells support breakpoints.
  Custom execution completes atomically within its caller's Global Tick;
  internal Custom cells cannot be paused independently and their breakpoints
  remain unverified. Inspect their committed events in Last Tick Events.
- The Call Stack shows Outer threads, their current cell, and Function frames.
- Variables show registers, selected thread state (including Page, direction,
  pointer, stacks, phase and PRNG state), sparse memory, input/output, metrics,
  errors and the previous tick's events.
- Watch and Debug Console evaluation accept read-only state paths such as
  `registers[0]`, `thread.data_stack`, `memory`, or `metrics.global_tick`.
  They do not execute expressions or mutate VM state.
- Source errors prevent execution. Runtime errors and tick/work limits stop a
  debug session with state available for inspection. Stop closes the process.

Configure defaults through `codegrid.execution.*` settings or use a launch
configuration for a particular program:

```json
{
  "version": "0.2.0",
  "configurations": [{
    "type": "codegrid",
    "request": "launch",
    "name": "Debug CodeGrid",
    "program": "${file}",
    "stopOnEntry": true,
    "input": [65],
    "boundary": "exit",
    "seed": "0",
    "customLimit": "10000",
    "maxTicks": "100000",
    "maxWorkUnits": "1000000"
  }]
}
```

Seed and limits use canonical decimal strings to preserve the entire `u64`
range. `maxWorkUnits` bounds each atomic Global Tick; `maxTicks` bounds the
session. These are local development safeguards, not a production sandbox.
Reverse execution, conditional breakpoints, data breakpoints and arbitrary
expression evaluation are not implemented.

### Features

- Registers `.cg` files as the `codegrid` language and supplies a file icon.
- Highlights the Full Primary inventory, Entry and Empty cells, ReadCode,
  WriteCode, and Repeat suffix attachments, conditional prefixes `?0`–`?2`,
  CMP `?=`, random direction `??`, Main/Custom/Function/Folded Block paths,
  named `@end` paths, dimensions, comments, and malformed lexical tokens.
- Completes Full Primary and cell spellings, grammar-level attachment forms,
  structural directive paths, and named `@end` shapes. Standalone suggestions
  are static: they do not resolve the current definition scope, match a closing
  name to the open block, check board-context restrictions, or verify that a
  referenced Function, Custom CodeGrid, or Folded Block exists.
- Provides English hover descriptions for known instruction metadata,
  directives, Entry markers, and incomplete multi-character prefixes.
- Formats complete cell rows conservatively. It preserves comments, cell
  order, row shape, CRLF or LF, and whether the file ends with a newline. The
  formatter does not perform source-level static validation and declines
  structure forms it cannot preserve safely.
- Provides `CodeGrid: New CodeGrid File` with a Main Board template and a
  minimal one-row template.
- Provides `CodeGrid: Learn about CodeGrid 0.1` and a language status item.

The formatter is a presentation helper. It does not validate dimensions,
Entry counts, definition scope, or instruction placement, and it leaves source
unchanged when it cannot safely understand a construct.

### Optional native language server

Set `codegrid.languageServer.enabled` to `true` and set
`codegrid.languageServer.path` to a built `codegrid-lsp` executable. The LSP
adds compiler-backed diagnostics, structural completion and hover, document
symbols, and source formatting for current unsaved documents. The VS Code
Outline shows source definitions. The server is off by default; when it cannot
start or exits, the extension restores its standalone providers.

Build the server from the repository root with:

```powershell
cargo build --release -p codegrid-lsp
```

Then set the executable path to `target/release/codegrid-lsp` or
`target/release/codegrid-lsp.exe`. The extension does not download or install
the server.

### Development and tests

```powershell
npm install        # once
npm run compile    # type-check and build to out/
npm test           # run integration tests in an Extension Development Host
npm run package    # produce a .vsix
```

For a Windows x64 VSIX that includes the current native execution runtime,
install dependencies first and run from the repository root:

```powershell
.\scripts\package_vscode.ps1
```

The package is written to `target/packages/codegrid-vscode-0.3.0-win32-x64.vsix`.
`npm run package` alone packages files already present and does not rebuild the
native executable. Packaging scripts must use a target that matches the native
binary they include.

The Extension Development Host tests cover language registration, Full
TextMate scopes, token and attachment completions, static structural and
named-close suggestions, hover, formatting preservation and idempotence, CRLF
and final-newline handling, and optional LSP fallback. When
`CODEGRID_LSP_TEST_SERVER` points to a built server and a CLI executable is
available beside it (or through `CODEGRID_CLI_TEST_BIN`), the suite also checks
real LSP completion, hover, source symbols, and diagnostic parity with
`codegrid check`.

With `CODEGRID_CLI_TEST_BIN`, the suite also exercises the actual native
debug transport: entry and tick stepping, source positions, column breakpoints,
Function step-out, Folded Blocks, simultaneous threads, watches and variables,
input/output, pause/stop, execution ceilings, source errors, and a real VS Code
debug-session launch. Tests that need the native executable are skipped when
that environment variable is absent.

### Source of truth

The Full source and VM specifications define accepted `.cg` syntax and
instruction behavior. This extension contains presentation metadata and
lexical editing helpers; the Rust compiler and language service remain the
authorities for validation and name resolution.

### License

BSD 3-Clause. See the LICENSE file shipped with the extension.

### Stable error identifiers

Source errors show the compiler code in Problems and the Debug Console. Runtime errors retain their VM code. Debug/transport/editor failures show a bracketed stable category; native debug protocol 2 uses structured code/message errors. The installation package includes the [complete error specification](runtime/codegrid-error-codes.md) and its machine-readable registry beside the bundled runtime. The authoritative repository files are `spec/codegrid-error-codes.md` and `spec/codegrid-error-codes.json`. Seed, limits, source acceptance and VM behavior are unchanged.
