// Run with: node tests/champions_reference.cjs /absolute/path/to/showdown/dist/sim
// The simulator is loaded read-only; no source or data files are changed.
const assert = require('node:assert/strict');
const path = require('node:path');
const {Battle, Dex} = require(path.resolve(process.argv[2]));
const formatid = 'gen9championsvgc2026regmc';
assert.equal(Dex.formats.get(formatid).mod, 'champions');
const move = Dex.forFormat(formatid).moves.get('makeitrain');
assert.equal(move.accuracy, 95);
assert.equal(move.self.boosts.spa, -2);

function scenario(move, setup, check) {
  const b = new Battle({formatid, seed: [1, 2, 3, 4]});
  const team = ['Gholdengo', 'Arcanine', 'Garchomp', 'Amoonguss'].map(species =>
    ({species, ability: 'No Ability', level: 50, moves: [move]}));
  b.setPlayer('p1', {name: 'one', team});
  b.setPlayer('p2', {name: 'two', team});
  b.choose('p1', 'team 1234');
  b.choose('p2', 'team 1234');
  const [a, ally] = b.p1.active;
  const [x, y] = b.p2.active;
  for (const p of [a, ally, x, y]) {
    p.hp = p.maxhp = p.baseMaxhp = 500;
    p.setType('Normal');
    p.storedStats = {atk: 100, def: 100, spa: 100, spd: 100, spe: 100};
  }
  // No critical hits. Explicit accuracy outcomes are controlled per scenario.
  b.randomChance = (n, d) => d === 24 ? false : true;
  setup(b, a, ally, x, y);
  b.actions.runMove(move, a, 1);
  check(b, a, ally, x, y);
  return {b, a, ally, x, y};
}

for (const protectedSlot of ['x', 'y']) {
  scenario('makeitrain', (b, a, ally, x, y) => {
    (protectedSlot === 'x' ? x : y).addVolatile('protect');
  }, (b, a) => assert.equal(a.boosts.spa, -2));
  scenario('dazzlinggleam', (b, a, ally, x, y) => {
    a.setItem('lifeorb');
    (protectedSlot === 'x' ? x : y).addVolatile('protect');
  }, (b, a) => assert.equal(a.hp, 450));
}
scenario('makeitrain', (b, a, ally, x, y) => {
  x.addVolatile('protect'); y.addVolatile('protect');
}, (b, a) => assert.equal(a.boosts.spa, 0));
scenario('makeitrain', b => {
  b.randomChance = () => false;
}, (b, a) => assert.equal(a.boosts.spa, 0));
for (const mv of ['dazzlinggleam', 'earthquake']) {
  scenario(mv, (b, a, ally, x, y) => {
    ally.hp = y.hp = 0; ally.fainted = y.fainted = true;
    b.p2.addSideCondition('wideguard', x);
  }, (b, a, ally, x) => assert.equal(x.hp, 500));
}
const guarded = partnerHP => scenario('dazzlinggleam', (b, a, ally, x) => {
  x.setAbility('friendguard'); x.hp = partnerHP;
}, () => {}).y.hp;
assert.equal(guarded(1), guarded(500));
for (const status of ['taunt', 'encore']) {
  const {b, a} = scenario('dazzlinggleam', (b, a) => {
    b.queue.clear();
    a.lastMove = b.dex.moves.get('dazzlinggleam');
    a.addVolatile(status);
  }, () => {});
  const remaining = a.volatiles[status].duration;
  b.fieldEvent('Residual');
  assert.equal(a.volatiles[status].duration, remaining - 1);
}
console.log('Champions reference checks passed: Make It Rain, partial failures, Life Orb, Wide Guard, Friend Guard, Taunt, Encore.');
