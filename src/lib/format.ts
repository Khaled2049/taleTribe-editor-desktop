const dateTime = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

export const formatDateTime = (ms: number) => dateTime.format(new Date(ms));

export const formatWords = (count: number) =>
  `${count.toLocaleString()} ${count === 1 ? "word" : "words"}`;
