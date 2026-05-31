import { Card } from "./Card";

export function ResultBlock({ title, value }: { title: string; value: unknown }) {
  return (
    <Card className="overflow-hidden p-0 gap-0">
      <Card.Header>
        <Card.Title>{title}</Card.Title>
      </Card.Header>
      <pre className="code-panel compact m-3 mt-3">{JSON.stringify(value, null, 2)}</pre>
    </Card>
  );
}
