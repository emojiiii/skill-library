import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { markdown } from "@codemirror/lang-markdown";
import { rust } from "@codemirror/lang-rust";
import { sql } from "@codemirror/lang-sql";
import type { Extension } from "@codemirror/state";

function getLanguageExtension(fileName: string): Extension[] {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? "";
  switch (ext) {
    case "js":
    case "jsx":
      return [javascript({ jsx: true })];
    case "ts":
    case "tsx":
      return [javascript({ jsx: true, typescript: true })];
    case "py":
      return [python()];
    case "json":
      return [json()];
    case "yaml":
    case "yml":
      return [yaml()];
    case "html":
    case "xml":
    case "svg":
      return [html()];
    case "css":
    case "scss":
      return [css()];
    case "md":
      return [markdown()];
    case "rs":
      return [rust()];
    case "sql":
      return [sql()];
    default:
      return [];
  }
}

export function CodeEditor({
  value,
  fileName,
  readOnly = false,
  onChange,
}: {
  value: string;
  fileName: string;
  readOnly?: boolean;
  onChange?: (value: string) => void;
}) {
  const extensions = getLanguageExtension(fileName);

  return (
    <CodeMirror
      value={value}
      height="100%"
      extensions={extensions}
      readOnly={readOnly}
      onChange={onChange}
      basicSetup={{
        lineNumbers: true,
        foldGutter: true,
        highlightActiveLine: !readOnly,
        highlightSelectionMatches: true,
        bracketMatching: true,
        autocompletion: false,
      }}
      className="code-editor-wrapper"
    />
  );
}
