export function ResultBlock({ title, value }: { title: string; value: unknown }) {
  return (
    <div className="card overflow-hidden">
      <div className="card-header">
        <div className="card-title">{title}</div>
      </div>
      <pre className="code-panel compact m-3 mt-3">{JSON.stringify(value, null, 2)}</pre>
    </div>
  );
}
