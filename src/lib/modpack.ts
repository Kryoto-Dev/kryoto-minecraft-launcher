/** A Modrinth modpack, which becomes an instance of its own when dropped. */
export const isModpack = (path: string) => path.toLowerCase().endsWith('.mrpack')
