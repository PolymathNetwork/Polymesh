(module
  (type $t0 (func (param i32 i32 i32) (result i32)))
  (type $t1 (func (param i32 i32 i32)))
  (type $t2 (func (param i32 i32)))
  (type $t3 (func (param i32) (result i32)))
  (type $t4 (func (param i32)))
  (type $t5 (func (result i32)))
  (type $t6 (func (param i32 i32 i32)))
  (import "seal0" "seal_get_storage" (func $seal0.seal_get_storage (type $t0)))
  (import "seal0" "seal_set_storage" (func $seal0.seal_set_storage (type $t1)))
  (import "seal0" "seal_value_transferred" (func $seal0.seal_value_transferred (type $t2)))
  (import "seal0" "seal_input" (func $seal0.seal_input (type $t2)))
  (import "seal0" "seal_return" (func $seal0.seal_return (type $t1)))
  (import "env" "memory" (memory $env.memory 2 16))
  (func $f5 (type $t3) (param $p0 i32) (result i32)
    (local $l1 i32) (local $l2 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l1
    global.set $g0
    local.get $l1
    i32.const 8
    i32.add
    local.get $p0
    call $f6
    local.get $l1
    i32.load8_u offset=9
    local.set $p0
    local.get $l1
    i32.load8_u offset=8
    local.set $l2
    local.get $l1
    i32.const 16
    i32.add
    global.set $g0
    i32.const 2
    i32.const 1
    i32.const 2
    local.get $p0
    i32.const 1
    i32.eq
    select
    i32.const 0
    local.get $p0
    select
    local.get $l2
    i32.const 1
    i32.and
    select)
  (func $f6 (type $t2) (param $p0 i32) (param $p1 i32)
    (local $l2 i32) (local $l3 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l2
    global.set $g0
    local.get $l2
    i32.const 0
    i32.store8 offset=15
    block $B0
      local.get $p1
      local.get $l2
      i32.const 15
      i32.add
      i32.const 1
      call $f18
      i32.eqz
      if $I1
        local.get $l2
        i32.load8_u offset=15
        local.set $p1
        br $B0
      end
      i32.const 1
      local.set $l3
    end
    local.get $p0
    local.get $p1
    i32.store8 offset=1
    local.get $p0
    local.get $l3
    i32.store8
    local.get $l2
    i32.const 16
    i32.add
    global.set $g0)
  (func $f7 (type $t2) (param $p0 i32) (param $p1 i32)
    (local $l2 i32) (local $l3 i32) (local $l4 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l2
    global.set $g0
    block $B0
      block $B1
        loop $L2
          local.get $l2
          local.get $l4
          i32.store8 offset=12
          local.get $l3
          i32.const 4
          i32.eq
          br_if $B1
          local.get $l2
          local.get $p1
          call $f6
          local.get $l2
          i32.load8_u
          i32.const 1
          i32.and
          i32.eqz
          if $I3
            local.get $l2
            i32.const 8
            i32.add
            local.get $l3
            i32.add
            local.get $l2
            i32.load8_u offset=1
            i32.store8
            local.get $l4
            i32.const 1
            i32.add
            local.set $l4
            local.get $l3
            i32.const 1
            i32.add
            local.set $l3
            br $L2
          end
        end
        local.get $p0
        i32.const 1
        i32.store8
        local.get $l3
        i32.const 255
        i32.and
        i32.eqz
        br_if $B0
        local.get $l2
        i32.const 0
        i32.store8 offset=12
        br $B0
      end
      local.get $p0
      local.get $l2
      i32.load offset=8
      i32.store offset=1 align=1
      local.get $p0
      i32.const 0
      i32.store8
    end
    local.get $l2
    i32.const 16
    i32.add
    global.set $g0)
  (func $f8 (type $t4) (param $p0 i32)
    (local $l1 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l1
    global.set $g0
    local.get $l1
    local.get $p0
    i32.store offset=12
    local.get $l1
    i32.const 12
    i32.add
    i32.load
    i32.load8_u
    call $f9
    unreachable)
  (func $f9 (type $t4) (param $p0 i32)
    (local $l1 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l1
    global.set $g0
    local.get $l1
    i32.const 16384
    i32.store offset=12
    local.get $l1
    i32.const 65572
    i32.store offset=8
    local.get $l1
    local.get $l1
    i32.const 8
    i32.add
    local.get $p0
    call $f10
    i32.const 0
    local.get $l1
    i32.load
    local.get $l1
    i32.load offset=4
    call $seal0.seal_return
    unreachable)
  (func $f10 (type $t1) (param $p0 i32) (param $p1 i32) (param $p2 i32)
    (local $l3 i32) (local $l4 i32) (local $l5 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l3
    global.set $g0
    local.get $p1
    i32.load offset=4
    local.set $l4
    local.get $p1
    i32.const 0
    i32.store offset=4
    local.get $p1
    i32.load
    local.set $l5
    local.get $p1
    i32.const 65572
    i32.store
    local.get $l3
    local.get $p2
    i32.store8 offset=15
    block $B0
      local.get $l4
      i32.const 1
      i32.ge_u
      if $I1
        local.get $l3
        i32.const 1
        i32.store offset=4
        local.get $l3
        local.get $l5
        i32.store
        br $B0
      end
      unreachable
    end
    block $B2
      local.get $l3
      i32.load offset=4
      i32.const 1
      i32.eq
      if $I3
        local.get $l3
        i32.load
        local.get $l3
        i32.load8_u offset=15
        i32.store8
        local.get $p1
        local.get $l4
        i32.store offset=4
        local.get $p1
        local.get $l5
        i32.store
        local.get $l4
        i32.eqz
        br_if $B2
        local.get $p1
        local.get $l4
        i32.const -1
        i32.add
        i32.store offset=4
        local.get $p1
        local.get $l5
        i32.const 1
        i32.add
        i32.store
        local.get $p0
        i32.const 1
        i32.store offset=4
        local.get $p0
        local.get $l5
        i32.store
        local.get $l3
        i32.const 16
        i32.add
        global.set $g0
        return
      end
      unreachable
    end
    unreachable)
  (func $f11 (type $t3) (param $p0 i32) (result i32)
    (local $l1 i32)
    global.get $g0
    i32.const -64
    i32.add
    local.tee $l1
    global.set $g0
    local.get $l1
    i32.const 32
    i32.add
    local.get $p0
    i32.const 24
    i32.add
    i64.load
    i64.store
    local.get $l1
    i32.const 24
    i32.add
    local.get $p0
    i32.const 16
    i32.add
    i64.load
    i64.store
    local.get $l1
    i32.const 16
    i32.add
    local.get $p0
    i32.const 8
    i32.add
    i64.load
    i64.store
    local.get $l1
    i64.const 0
    i64.store offset=40
    local.get $l1
    local.get $p0
    i64.load
    i64.store offset=8
    local.get $l1
    i32.const 8
    i32.add
    call $f12
    local.get $l1
    i32.const 16384
    i32.store offset=52
    local.get $l1
    i32.const 65572
    i32.store offset=48
    local.get $l1
    i32.const 16384
    i32.store offset=56
    i32.const 65572
    local.get $l1
    i32.const 56
    i32.add
    call $seal0.seal_get_storage
    local.set $p0
    local.get $l1
    i32.const 48
    i32.add
    local.get $l1
    i32.load offset=56
    call $f13
    block $B0
      block $B1
        block $B2
          block $B3
            local.get $p0
            br_table $B2 $B3 $B3 $B1 $B3
          end
          unreachable
        end
        local.get $l1
        local.get $l1
        i64.load offset=48
        i64.store offset=56
        local.get $l1
        i32.const 56
        i32.add
        call $f5
        i32.const 255
        i32.and
        local.tee $p0
        i32.const 2
        i32.ne
        br_if $B0
        unreachable
      end
      unreachable
    end
    local.get $l1
    i32.const -64
    i32.sub
    global.set $g0
    local.get $p0
    i32.const 0
    i32.ne)
  (func $f12 (type $t3) (param $p0 i32) (result i32)
    (local $l1 i64) (local $l2 i64) (local $l3 i64)
    local.get $p0
    i64.load offset=32
    local.set $l1
    local.get $p0
    i64.const 1
    i64.store offset=32
    local.get $p0
    local.get $l1
    local.get $p0
    i64.load
    local.tee $l2
    i64.add
    local.tee $l1
    i64.store
    local.get $p0
    local.get $p0
    i64.load offset=8
    local.tee $l3
    local.get $l1
    local.get $l2
    i64.lt_u
    i64.extend_i32_u
    i64.add
    local.tee $l1
    i64.store offset=8
    local.get $p0
    local.get $p0
    i64.load offset=16
    local.tee $l2
    local.get $l1
    local.get $l3
    i64.lt_u
    i64.extend_i32_u
    i64.add
    local.tee $l1
    i64.store offset=16
    local.get $p0
    local.get $p0
    i64.load offset=24
    local.get $l1
    local.get $l2
    i64.lt_u
    i64.extend_i32_u
    i64.add
    i64.store offset=24
    local.get $p0)
  (func $f13 (type $t2) (param $p0 i32) (param $p1 i32)
    (local $l2 i32) (local $l3 i32) (local $l4 i32) (local $l5 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l2
    global.set $g0
    local.get $p0
    i32.load offset=4
    local.set $l4
    local.get $p0
    i32.const 0
    i32.store offset=4
    local.get $p0
    i32.load
    local.set $l5
    local.get $p0
    i32.const 65572
    i32.store
    local.get $l2
    i32.const 8
    i32.add
    local.set $l3
    block $B0
      i32.const 0
      local.get $p1
      i32.le_u
      if $I1
        local.get $l4
        local.get $p1
        i32.ge_u
        if $I2
          local.get $l3
          local.get $p1
          i32.store offset=4
          local.get $l3
          local.get $l5
          i32.store
          br $B0
        end
        unreachable
      end
      unreachable
    end
    local.get $p0
    local.get $l2
    i64.load offset=8
    i64.store align=4
    local.get $l2
    i32.const 16
    i32.add
    global.set $g0)
  (func $f14 (type $t2) (param $p0 i32) (param $p1 i32)
    (local $l2 i32)
    global.get $g0
    i32.const -64
    i32.add
    local.tee $l2
    global.set $g0
    local.get $l2
    i32.const 40
    i32.add
    local.get $p1
    i32.const 24
    i32.add
    i64.load
    i64.store
    local.get $l2
    i32.const 32
    i32.add
    local.get $p1
    i32.const 16
    i32.add
    i64.load
    i64.store
    local.get $l2
    i32.const 24
    i32.add
    local.get $p1
    i32.const 8
    i32.add
    i64.load
    i64.store
    local.get $l2
    i64.const 0
    i64.store offset=48
    local.get $l2
    local.get $p1
    i64.load
    i64.store offset=16
    local.get $l2
    i32.const 16
    i32.add
    call $f12
    local.get $l2
    i32.const 16384
    i32.store offset=60
    local.get $l2
    i32.const 65572
    i32.store offset=56
    local.get $l2
    i32.const 8
    i32.add
    local.get $l2
    i32.const 56
    i32.add
    local.get $p0
    call $f10
    local.get $l2
    i32.load offset=8
    local.get $l2
    i32.load offset=12
    call $seal0.seal_set_storage
    local.get $l2
    i32.const -64
    i32.sub
    global.set $g0)
  (func $deploy (type $t5) (result i32)
    i32.const 0
    call $f16
    i32.const 255
    i32.and
    i32.const 2
    i32.shl
    i32.const 65536
    i32.add
    i32.load)
  (func $f16 (type $t3) (param $p0 i32) (result i32)
    (local $l1 i32) (local $l2 i32) (local $l3 i32) (local $l4 i32) (local $l5 i32)
    global.get $g0
    i32.const 48
    i32.sub
    local.tee $l1
    global.set $g0
    block $B0 (result i32)
      block $B1
        block $B2
          local.get $p0
          if $I3
            local.get $l1
            i32.const 16384
            i32.store offset=4
            local.get $l1
            i32.const 65572
            i32.store
            local.get $l1
            call $f19
            local.get $l1
            local.get $l1
            i64.load
            i64.store offset=16
            local.get $l1
            i32.const 8
            i32.add
            local.get $l1
            i32.const 16
            i32.add
            call $f7
            i32.const 1
            local.set $l3
            local.get $l1
            i32.load8_u offset=8
            i32.const 1
            i32.eq
            if $I4
              i32.const 1
              local.set $l2
              br $B1
            end
            local.get $l1
            i32.load8_u offset=12
            local.set $p0
            local.get $l1
            i32.load8_u offset=11
            local.set $l2
            local.get $l1
            i32.load8_u offset=10
            local.set $l4
            local.get $l1
            i32.load8_u offset=9
            local.tee $l5
            i32.const 30
            i32.ne
            if $I5
              local.get $l5
              i32.const 192
              i32.ne
              br_if $B2
              local.get $l4
              i32.const 150
              i32.ne
              br_if $B2
              local.get $l2
              i32.const 165
              i32.ne
              br_if $B2
              i32.const 0
              local.set $l2
              local.get $p0
              i32.const 243
              i32.eq
              br_if $B1
              br $B2
            end
            local.get $l4
            i32.const 92
            i32.ne
            br_if $B2
            local.get $l2
            i32.const 164
            i32.ne
            br_if $B2
            i32.const 0
            local.set $l2
            i32.const 0
            local.set $l3
            local.get $p0
            i32.const 86
            i32.ne
            br_if $B2
            br $B1
          end
          local.get $l1
          i32.const 16384
          i32.store offset=4
          local.get $l1
          i32.const 65572
          i32.store
          local.get $l1
          call $f19
          local.get $l1
          local.get $l1
          i64.load
          i64.store offset=16
          local.get $l1
          i32.const 8
          i32.add
          local.get $l1
          i32.const 16
          i32.add
          call $f7
          i32.const 3
          local.set $l2
          block $B6
            local.get $l1
            i32.load8_u offset=8
            i32.const 1
            i32.eq
            br_if $B6
            local.get $l1
            i32.load8_u offset=12
            local.set $p0
            local.get $l1
            i32.load8_u offset=11
            local.set $l2
            local.get $l1
            i32.load8_u offset=10
            local.set $l3
            block $B7
              block $B8
                local.get $l1
                i32.load8_u offset=9
                local.tee $l4
                i32.const 106
                i32.ne
                if $I9
                  local.get $l4
                  i32.const 209
                  i32.ne
                  br_if $B7
                  local.get $l3
                  i32.const 131
                  i32.ne
                  br_if $B7
                  local.get $l2
                  i32.const 81
                  i32.ne
                  br_if $B7
                  local.get $p0
                  i32.const 43
                  i32.eq
                  br_if $B8
                  br $B7
                end
                local.get $l3
                i32.const 55
                i32.ne
                br_if $B7
                local.get $l2
                i32.const 18
                i32.ne
                br_if $B7
                i32.const 2
                local.set $l2
                local.get $p0
                i32.const 226
                i32.ne
                br_if $B7
                br $B6
              end
              i32.const 3
              local.set $l2
              local.get $l1
              i32.const 16
              i32.add
              call $f5
              i32.const 255
              i32.and
              local.tee $p0
              i32.const 2
              i32.eq
              br_if $B6
              local.get $p0
              i32.const 0
              i32.ne
              local.set $l2
              br $B6
            end
            i32.const 3
            local.set $l2
          end
          i32.const 6
          local.get $l2
          i32.const 3
          i32.eq
          local.tee $l3
          br_if $B0
          drop
          i32.const 6
          local.get $l2
          local.get $l3
          select
          local.tee $l2
          i32.const 2
          i32.eq
          if $I10
            local.get $l1
            i32.const 40
            i32.add
            i64.const 0
            i64.store
            local.get $l1
            i32.const 32
            i32.add
            i64.const 0
            i64.store
            local.get $l1
            i32.const 24
            i32.add
            i64.const 0
            i64.store
            local.get $l1
            i64.const 0
            i64.store offset=16
            i32.const 0
            local.get $l1
            i32.const 16
            i32.add
            call $f14
            i32.const 8
            br $B0
          end
          local.get $l1
          i32.const 40
          i32.add
          i64.const 0
          i64.store
          local.get $l1
          i32.const 32
          i32.add
          i64.const 0
          i64.store
          local.get $l1
          i32.const 24
          i32.add
          i64.const 0
          i64.store
          local.get $l1
          i64.const 0
          i64.store offset=16
          local.get $l2
          i32.const 1
          i32.and
          local.get $l1
          i32.const 16
          i32.add
          call $f14
          i32.const 8
          br $B0
        end
        i32.const 1
        local.set $l2
        i32.const 1
        local.set $l3
      end
      i32.const 6
      local.get $l2
      br_if $B0
      drop
      local.get $l3
      i32.eqz
      if $I11
        local.get $l1
        i32.const 40
        i32.add
        i64.const 0
        i64.store
        local.get $l1
        i32.const 32
        i32.add
        i64.const 0
        i64.store
        local.get $l1
        i32.const 24
        i32.add
        i64.const 0
        i64.store
        local.get $l1
        i64.const 0
        i64.store offset=16
        local.get $l1
        local.get $l1
        i32.const 16
        i32.add
        call $f11
        i32.store8
        local.get $l1
        call $f8
        unreachable
      end
      local.get $l1
      i32.const 40
      i32.add
      i64.const 0
      i64.store
      local.get $l1
      i32.const 32
      i32.add
      i64.const 0
      i64.store
      local.get $l1
      i32.const 24
      i32.add
      i64.const 0
      i64.store
      local.get $l1
      i64.const 0
      i64.store offset=16
      local.get $l1
      i32.const 16
      i32.add
      call $f11
      i32.const 1
      i32.xor
      local.get $l1
      i32.const 16
      i32.add
      call $f14
      i32.const 8
    end
    local.get $l1
    i32.const 48
    i32.add
    global.set $g0)
  (func $call (type $t5) (result i32)
    (local $l0 i32) (local $l1 i32) (local $l2 i64) (local $l3 i64)
    global.get $g0
    i32.const 32
    i32.sub
    local.tee $l0
    global.set $g0
    local.get $l0
    i32.const 16384
    i32.store offset=4
    local.get $l0
    i32.const 65572
    i32.store
    local.get $l0
    i32.const 16384
    i32.store offset=16
    i32.const 65572
    local.get $l0
    i32.const 16
    i32.add
    call $seal0.seal_value_transferred
    local.get $l0
    local.get $l0
    i32.load offset=16
    call $f13
    local.get $l0
    local.get $l0
    i64.load
    i64.store offset=8
    local.get $l0
    i32.const 24
    i32.add
    local.tee $l1
    i64.const 0
    i64.store
    local.get $l0
    i64.const 0
    i64.store offset=16
    block $B0
      block $B1 (result i32)
        i32.const 1
        local.get $l0
        i32.const 8
        i32.add
        local.get $l0
        i32.const 16
        i32.add
        i32.const 16
        call $f18
        br_if $B1
        drop
        local.get $l1
        i64.load
        local.set $l2
        local.get $l0
        i64.load offset=16
        local.set $l3
        i32.const 0
      end
      br_if $B0
      local.get $l2
      local.get $l3
      i64.or
      i64.eqz
      i32.eqz
      br_if $B0
      i32.const 1
      call $f16
      local.get $l0
      i32.const 32
      i32.add
      global.set $g0
      i32.const 255
      i32.and
      i32.const 2
      i32.shl
      i32.const 65536
      i32.add
      i32.load
      return
    end
    unreachable)
  (func $f18 (type $t0) (param $p0 i32) (param $p1 i32) (param $p2 i32) (result i32)
    block $B0
      local.get $p0
      i32.load offset=4
      local.get $p2
      i32.ge_u
      if $I1 (result i32)
        local.get $p1
        local.get $p0
        i32.load
        local.get $p2
        call $f20
        local.get $p0
        i32.load offset=4
        local.tee $p1
        local.get $p2
        i32.lt_u
        br_if $B0
        local.get $p0
        local.get $p1
        local.get $p2
        i32.sub
        i32.store offset=4
        local.get $p0
        local.get $p0
        i32.load
        local.get $p2
        i32.add
        i32.store
        i32.const 0
      else
        i32.const 1
      end
      return
    end
    unreachable)
  (func $f19 (type $t4) (param $p0 i32)
    (local $l1 i32)
    global.get $g0
    i32.const 16
    i32.sub
    local.tee $l1
    global.set $g0
    local.get $l1
    local.get $p0
    i32.load offset=4
    i32.store offset=12
    local.get $p0
    i32.load
    local.get $l1
    i32.const 12
    i32.add
    call $seal0.seal_input
    local.get $p0
    local.get $l1
    i32.load offset=12
    call $f13
    local.get $l1
    i32.const 16
    i32.add
    global.set $g0)
  (func $f20 (type $t6) (param $p0 i32) (param $p1 i32) (param $p2 i32)
    (local $l3 i32)
    loop $L0 (result i32)
      local.get $p2
      local.get $l3
      i32.eq
      if $I1 (result i32)
        local.get $p0
      else
        local.get $p0
        local.get $l3
        i32.add
        local.get $p1
        local.get $l3
        i32.add
        i32.load8_u
        i32.store8
        local.get $l3
        i32.const 1
        i32.add
        local.set $l3
        br $L0
      end
    end
    drop)
  (global $g0 (mut i32) (i32.const 65536))
  (export "deploy" (func $deploy))
  (export "call" (func $call))
  (data $d0 (i32.const 65536) "\01\00\00\00\02\00\00\00\03\00\00\00\04\00\00\00\05\00\00\00\06\00\00\00\07\00\00\00\08"))
