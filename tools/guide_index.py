"""The W guide's index, in the form the kit reads (AE2's, as Elevator's): pages written with a small front matter
(`navigation: title:`, `item_ids:` and `requires:`) become Markdown without it, and an index of item → page, titles,
the first page as the others' parent, positions, icons and the other mods a page needs."""
import json


def front_matter(text):
    """(title, item ids, required items, the page without its front matter). `requires:` lists an item of each other
    mod a page is about: the guide shows the page only when they are all installed."""
    if not text.startswith('---\n'):
        return None, [], [], text
    head, body = text[4:].split('\n---\n', 1)
    title, lists, in_list = None, {'item_ids': [], 'requires': []}, None
    for line in head.splitlines():
        s = line.strip()
        if s.startswith('title:'):
            title = s.split(':', 1)[1].strip()
        if s.endswith(':') and s[:-1] in lists:
            in_list = s[:-1]
            continue
        if in_list and s.startswith('- '):
            lists[in_list].append(s[2:].strip())
        elif in_list and s and not s.startswith('- '):
            in_list = None
    return title, lists['item_ids'], lists['requires'], body.lstrip('\n')


def if_mods(items, text):
    """A part of a page shown only when every item of `items` (one per other mod) is in the game: how mods work
    together, hidden when one of them is missing."""
    return f'<IfMods items="{",".join(items)}">\n{text}</IfMods>\n'


def scene(files, ns, name, blocks):
    """A 3D scene for `<GameScene><ImportStructure src="scenes/NAME.snbt" /></GameScene>`: its blocks as
    [x, y, z, id, {property: value}], the form the kit's guide reads (it turns with the mouse)."""
    rows = [[x, y, z, b] + ([p] if p else []) for (x, y, z, b, p) in ((*b, None) if len(b) == 4 else b for b in blocks)]
    files[f'data/{ns}/ae2guide/scenes/{name}.json'] = json.dumps({'blocks': rows}).encode()
    return f'<GameScene zoom="3" interactive={{true}}>\n  <ImportStructure src="scenes/{name}.snbt" />\n</GameScene>\n'


def write(files, ns, pages):
    """Puts `pages` (name → text with front matter) and their index into `files` under data/<ns>/ae2guide."""
    index = {'items': {}, 'titles': {}, 'parents': {}, 'positions': {}, 'icons': {}, 'categories': {}, 'requires': {}}
    names = list(pages)
    for at, name in enumerate(names):
        title, items, requires, body = front_matter(pages[name])
        if requires:
            index['requires'][name] = requires
        files[f'data/{ns}/ae2guide/{name}'] = body.encode()
        index['titles'][name] = title or name.removesuffix('.md')
        index['positions'][name] = str(at * 10)
        if at:
            index['parents'][name] = names[0]
        for item in items:
            index['items'].setdefault(item, name)
        if items:
            index['icons'][name] = items[0]
    files[f'data/{ns}/ae2guide/index.json'] = json.dumps(index, indent=2).encode()
    return index
