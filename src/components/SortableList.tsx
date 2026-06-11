import { ReactNode } from "react";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DraggableAttributes,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
  rectSortingStrategy,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

/** Drag activator props — spread onto the element that should start the drag (handle). */
export interface DragHandleProps {
  ref: (el: HTMLElement | null) => void;
  attributes: DraggableAttributes;
  listeners: ReturnType<typeof useSortable>["listeners"];
  isDragging: boolean;
}

interface SortableRowProps<T> {
  item: T;
  renderItem: (item: T, handle: DragHandleProps) => ReactNode;
}

function SortableRow<T extends { id: string }>({ item, renderItem }: SortableRowProps<T>) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: item.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 10 : undefined,
    position: "relative" as const,
  };

  const handle: DragHandleProps = {
    ref: setActivatorNodeRef,
    attributes,
    listeners,
    isDragging,
  };

  return (
    <div ref={setNodeRef} style={style}>
      {renderItem(item, handle)}
    </div>
  );
}

interface SortableListProps<T> {
  /** Items to sort; each MUST have a stable string `id`. */
  items: T[];
  /** Called with the reordered array after a drop. */
  onReorder: (next: T[]) => void;
  /** Render one row; spread `handle.attributes`/`handle.listeners` + set `handle.ref` on the drag handle element. */
  renderItem: (item: T, handle: DragHandleProps) => ReactNode;
  /** "vertical" (default) for lists, "grid" for wrapping multi-column layouts. */
  strategy?: "vertical" | "grid";
}

/**
 * Generic @dnd-kit sortable list. Single source of truth for all drag-reorder
 * UIs in the app (statusline segments / tray columns / group platforms).
 */
export function SortableList<T extends { id: string }>({
  items,
  onReorder,
  renderItem,
  strategy = "vertical",
}: SortableListProps<T>) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const handleDragEnd = (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const from = items.findIndex((i) => i.id === active.id);
    const to = items.findIndex((i) => i.id === over.id);
    if (from < 0 || to < 0) return;
    onReorder(arrayMove(items, from, to));
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext
        items={items.map((i) => i.id)}
        strategy={strategy === "grid" ? rectSortingStrategy : verticalListSortingStrategy}
      >
        {items.map((item) => (
          <SortableRow key={item.id} item={item} renderItem={renderItem} />
        ))}
      </SortableContext>
    </DndContext>
  );
}
