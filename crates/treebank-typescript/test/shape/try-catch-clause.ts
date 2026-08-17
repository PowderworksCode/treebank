// The catch clause belongs to the try statement. It used to parse as a
// separate function declaration named `catch`, leaving the try empty.
export async function run(schema: (v: unknown) => unknown, values: unknown) {
  try {
    const data = await schema(values);

    return data;
  } catch (error: any) {
    return error;
  } finally {
    report();
  }
}
declare function report(): void;
