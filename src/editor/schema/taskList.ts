import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";

export const TaskListExtension = TaskList.configure({
  HTMLAttributes: { class: "ns-task-list" },
});

export const TaskItemExtension = TaskItem.configure({
  nested: true,
  HTMLAttributes: { class: "ns-task-item" },
});
