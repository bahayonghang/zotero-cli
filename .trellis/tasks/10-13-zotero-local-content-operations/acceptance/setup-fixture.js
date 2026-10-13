// Run in Zotero's Tools > Developer > Run JavaScript (async mode).
// Only creates synthetic fixtures inside the isolated acceptance profile.
const dataDir = Zotero.DataDirectory.dir.replaceAll("\\", "/");
if (!dataDir.endsWith("/target/local-api-acceptance/01a134db/data")) {
  throw new Error("Refusing fixtures outside the isolated acceptance directory");
}
const libraryID = Zotero.Libraries.userLibraryID;
const existing = await Zotero.Items.getAll(libraryID);
if (existing.length) throw new Error("Expected an empty acceptance library");
const collections = {};
for (const name of ["A", "B", "C"]) {
  const collection = new Zotero.Collection();
  collection.libraryID = libraryID;
  collection.name = "CLI acceptance " + name;
  await collection.saveTx();
  collections[name] = collection.key;
}
const item = new Zotero.Item("conferencePaper");
item.libraryID = libraryID;
item.setField("title", "CLI acceptance synthetic paper");
item.setField("extra", "Synthetic test fixture. Not a real publication.");
item.setCollections([collections.A, collections.C]);
await item.saveTx();
const note = new Zotero.Item("note");
note.parentID = item.id;
note.setNote("<p>Acceptance child note: preserve this content.</p>");
await note.saveTx();
const attachment = await Zotero.Attachments.linkFromURL({
  parentItemID: item.id,
  url: "https://example.invalid/cli-acceptance.pdf",
  title: "Acceptance linked attachment",
  contentType: "application/pdf",
});
const fixture = {
  data_dir: dataDir,
  item: item.key,
  collections,
  children: [note.key, attachment.key],
};
const manifestPath = PathUtils.join(PathUtils.parent(Zotero.DataDirectory.dir), "fixture.json");
await Zotero.File.putContentsAsync(manifestPath, JSON.stringify(fixture, null, 2));
await Zotero.getActiveZoteroPane().selectItem(item.id);
return fixture;
