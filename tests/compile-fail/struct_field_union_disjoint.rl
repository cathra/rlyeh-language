// U4（2026-08-31）：字段联合成员须两两互不相交，重叠报 UnionMembersNotDisjoint。
// expect: are not disjoint
struct S {
    id: i64 | isize,
}
