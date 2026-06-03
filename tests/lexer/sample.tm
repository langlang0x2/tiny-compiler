* TINY Compilation to TM Code
* Standard prelude:
0:  LD 6,0(0)    load gp with maxaddress
1:  LDA 0,0(6)    clear accumulator
* End of standard prelude.
  2:  IN 0,0,0    read integer value
  3:  ST 0,0(6)    read: x
  4:  LDC 0,0(0)    load const
  5:  ST 0,-2(6)    op: push left
  6:  LD 0,0(6)    load id: x
  7:  LD 1,-2(6)    op: load left
  8:  SUB 0,1,0    op cmp
  9:  JLT 0,2(7)    true case
 10:  LDC 0,0(0)    false case
 11:  LDA 7,1(7)    skip true case
 12:  LDC 0,1(0)    true case
 13:  JEQ 0,26(7)    if: jmp to end
 14:  LDC 0,1(0)    load const
 15:  ST 0,-1(6)    assign: fact
 16:  LD 0,-1(6)    load id: fact
 17:  ST 0,-3(6)    op: push left
 18:  LD 0,0(6)    load id: x
 19:  LD 1,-3(6)    op: load left
 20:  MUL 0,1,0    op *
 21:  ST 0,-1(6)    assign: fact
 22:  LD 0,0(6)    load id: x
 23:  ST 0,-3(6)    op: push left
 24:  LDC 0,1(0)    load const
 25:  LD 1,-3(6)    op: load left
 26:  SUB 0,1,0    op -
 27:  ST 0,0(6)    assign: x
 28:  LD 0,0(6)    load id: x
 29:  ST 0,-3(6)    op: push left
 30:  LDC 0,0(0)    load const
 31:  LD 1,-3(6)    op: load left
 32:  SUB 0,1,0    op cmp
 33:  JEQ 0,2(7)    true case
 34:  LDC 0,0(0)    false case
 35:  LDA 7,1(7)    skip true case
 36:  LDC 0,1(0)    true case
 37:  JEQ 0,-22(7)    repeat: jmp back
 38:  LD 0,-1(6)    load id: fact
 39:  OUT 0,0,0    write ac
* End of execution.
 40:  HALT 0,0,0
