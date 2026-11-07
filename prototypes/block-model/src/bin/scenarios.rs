use block_model::{snapshot, sync, EmbedModel, ListModel, Model, TreeModel};

fn pair<M: Model>(texts: &[&str]) -> (M, M) {
    let mut first = M::new();
    for text in texts {
        first.push(text);
    }
    let second = M::from_bytes(&snapshot(&first));
    (first, second)
}

fn run<M: Model>(name: &str, texts: &[&str], left: impl Fn(&mut M), right: impl Fn(&mut M)) {
    let (mut first, mut second) = pair::<M>(texts);
    left(&mut first);
    right(&mut second);
    sync(&first, &second);
    let converged = first.blocks() == second.blocks();
    println!("{:6} {:30} {} {:?}", M::NAME, name, if converged { "same" } else { "DIFFERENT" }, first.blocks());
}

fn all<M: Model>() {
    run::<M>("edit during move", &["one", "two", "three"], |m| m.relocate(0, 2), |m| m.edit(0, 3, " edited"));
    run::<M>("two moves of one block", &["one", "two", "three"], |m| m.relocate(0, 2), |m| m.relocate(0, 1));
    run::<M>("Enter, then add at the end", &["hello world"], |m| m.split(0, 6), |m| m.edit(0, 11, "!"));
    run::<M>("Enter, then add in the head", &["hello world"], |m| m.split(0, 6), |m| m.edit(0, 1, "E"));
    run::<M>("Enter, then delete the tail", &["hello world"], |m| m.split(0, 6), |m| m.delete(0, 6, 5));
    run::<M>("Enter at the same place", &["hello world"], |m| m.split(0, 6), |m| m.split(0, 6));
    run::<M>("Enter at different places", &["one two three"], |m| m.split(0, 4), |m| m.split(0, 8));
    run::<M>("merge, then add to the next", &["one", "two"], |m| m.merge(0), |m| m.edit(1, 3, "!"));
    run::<M>("merge at the same place", &["one", "two"], |m| m.merge(0), |m| m.merge(0));
}

fn main() {
    all::<ListModel>();
    println!();
    all::<TreeModel>();
    println!();
    all::<EmbedModel>();
    println!();

    let mut first = EmbedModel::new();
    first.push("intro");
    let id = first.push_rich("| table |");
    first.push("outro");
    let mut second = EmbedModel::from_bytes(&snapshot(&first));
    second.edit_rich(&id, 9, " edited");
    first.relocate(1, 3);
    sync(&first, &second);
    println!("embed  rich block: edit during move  {:?}", first.blocks());

    let mut first = EmbedModel::new();
    first.push("intro");
    first.push_rich("| table |");
    first.push("outro");
    let mut second = EmbedModel::from_bytes(&snapshot(&first));
    first.relocate(1, 3);
    second.relocate(1, 0);
    sync(&first, &second);
    println!("embed  rich block: two moves         {:?}", first.blocks());

    let mut model = EmbedModel::new();
    model.push("alpha");
    model.push("beta");
    let before = model.ids();
    model.edit(0, 0, "x");
    let reloaded = EmbedModel::from_bytes(&snapshot(&model));
    println!("embed  IDs after edit and reload     {}", before == reloaded.ids());
    model.split(0, 3);
    println!("embed  IDs after Enter               {:?} from {:?}", model.ids(), before);
}
