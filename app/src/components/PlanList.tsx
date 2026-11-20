import type { Task } from '../beacon';
import { Icon } from './Icon';
import { Spinner } from './Spinner';

export function PlanList({ tasks }: { tasks: Task[] }) {
  const currentIndex = tasks.findIndex((task) => !task.done);
  return (
    <section>
      <h3 className="mb-1 text-micro text-faint">Plan</h3>
      <ol className="space-y-0.5">
        {tasks.map((task, index) => (
          <li key={task.id} className="flex items-center gap-2 text-secondary">
            {task.done ? (
              <Icon name="check" className="size-3 shrink-0 text-muted" />
            ) : index === currentIndex ? (
              <Spinner />
            ) : (
              <Icon name="box" className="size-3 shrink-0 text-muted" />
            )}
            <span className={`min-w-0 truncate ${index === currentIndex ? 'text-ink' : 'text-muted'}`}>{task.subject}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}
