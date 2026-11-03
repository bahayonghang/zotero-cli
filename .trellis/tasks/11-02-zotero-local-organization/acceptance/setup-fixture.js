// Run once in Zotero Tools > Developer > Run JavaScript, async mode.
const expected = "D:/Documents/Code/CLI/zotero-cli/target/local-organization-acceptance/2026-11-02/data";
const dataDir = Zotero.DataDirectory.dir.replaceAll("\\", "/");
if (dataDir.toLowerCase() !== expected.toLowerCase()) throw new Error("Wrong data directory: " + dataDir);
const libraryID = Zotero.Libraries.userLibraryID;
if ((await Zotero.Items.getAll(libraryID)).length || Zotero.Collections.getByLibrary(libraryID).length) {
  throw new Error("Expected a completely empty isolated library; do not rerun");
}
const collections = {};
for (const name of ["Control", "Source"]) {
  const collection = new Zotero.Collection();
  collection.libraryID = libraryID;
  collection.name = "Organization acceptance " + name;
  await collection.saveTx();
  collections[name] = collection.key;
}
const items = [];
for (let index = 1; index <= 3; index++) {
  const item = new Zotero.Item("journalArticle");
  item.libraryID = libraryID;
  item.setField("title", "Organization acceptance paper " + index);
  item.setField("extra", "Synthetic organization acceptance. Not a real publication.");
  item.setField("date", "2026");
  item.setTags([{tag:"automatic-control",type:1},{tag:"manual-control",type:0},{tag:"obsolete",type:0}]);
  item.setCollections([collections.Control, collections.Source]);
  await item.saveTx();
  items.push(item.key);
}
const first = Zotero.Items.getByLibraryAndKey(libraryID, items[0]);
const note = new Zotero.Item("note");
note.parentID = first.id;
note.setNote("<p>Synthetic child note: preserve exactly.</p>");
await note.saveTx();
const attachment = await Zotero.Attachments.linkFromURL({
  parentItemID:first.id, url:"https://example.invalid/organization-acceptance.pdf",
  title:"Synthetic linked attachment", contentType:"application/pdf"
});
const fixture = {data_dir:dataDir, items, collections, children:[note.key,attachment.key]};
await Zotero.File.putContentsAsync(PathUtils.join(PathUtils.parent(Zotero.DataDirectory.dir),"fixture.json"), JSON.stringify(fixture,null,2));
await Zotero.getActiveZoteroPane().selectItem(first.id);
return fixture;
