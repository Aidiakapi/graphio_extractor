local player

local main_gui, start_export
script.on_event(defines.events.on_player_created, function(event)
    player = game.players[event.player_index]
    if player.character then
        local character = player.character
        player.character = nil
        character.destroy()
    end

    main_gui = player.gui.left.add({
        type = 'frame',
        direction = 'vertical',
        caption = 'Graphio image exporting',
    })

    player.surface.always_day = true
    local area = {{-100, -100}, {100, 100}}
    for _, entity in ipairs(player.surface.find_entities(area)) do
        entity.destroy()
    end
    player.surface.destroy_decoratives(area)

    main_gui.add({
        type = 'label',
        caption = 'Step 1. Set UI scale to 100%.'
    })
    main_gui.add({
        type = 'label',
        caption = 'Step 2. Disable clouds in graphics settings.'
    })
    main_gui.add({
        type = 'label',
        caption = "Step 3. Click start. This might take a while."
    })

    local start_button = main_gui.add({
        type = 'button',
        caption = 'Start',
    })

    script.on_event(defines.events.on_gui_click, function (event)
        if event.element ~= start_button then return end
        start_export()
    end)

    game.show_message_dialog{ text = "Follow the instructions on the left side of the screen." }
end)

start_export = function ()
    main_gui.destroy()
    main_gui = nil
    player.gui.left.clear()
    player.gui.top.clear()
    player.gui.center.clear()

    player.teleport({0,0})
    player.zoom = 1

    local tiles = {}
    for x = -100, 100 do
        for y = -100, 100 do
            tiles[#tiles + 1] = {
                position = {x, y},
                name = 'lab-white'
            }
        end
    end
    player.surface.set_tiles(tiles, true)

    local frames = {}

    for _, item in ipairs(extract_data.items) do
        frames[#frames + 1] = {
            path_light = output_folder .. '/light/items/' .. item .. '.png',
            path_dark = output_folder .. '/dark/items/' .. item .. '.png',
            sprite = 'item/' .. item,
        }
    end
    for _, fluid in ipairs(extract_data.fluids) do
        frames[#frames + 1] = {
            path_light = output_folder .. '/light/fluids/' .. fluid .. '.png',
            path_dark = output_folder .. '/dark/fluids/' .. fluid .. '.png',
            sprite = 'fluid/' .. fluid,
        }
    end
    for _, recipe in ipairs(extract_data.recipes) do
        frames[#frames + 1] = {
            path_light = output_folder .. '/light/recipes/' .. recipe .. '.png',
            path_dark = output_folder .. '/dark/recipes/' .. recipe .. '.png',
            sprite = 'recipe/' .. recipe,
        }
    end
    for _, entity in ipairs(extract_data.entities) do
        frames[#frames + 1] = {
            path_light = output_folder .. '/light/entities/' .. entity .. '.png',
            path_dark = output_folder .. '/dark/entities/' .. entity .. '.png',
            sprite = 'entity/' .. entity,
        }
    end
    

    for i = 1, #frames do
        local frame = frames[i]
        frames[i] = {
            path = frame.path_light,
            sprite = frame.sprite,
        }
        frames[#frames + 1] = {
            path = frame.path_dark,
            sprite = frame.sprite,
        }
    end
    frames[#frames / 2 + 1].switch = true

    local frame_index = 0
    local wait_frames = math.min(extract_interval * 10 + 30, 300)

    script.on_event(defines.events.on_tick, function (event)
        if wait_frames > 0 then
            wait_frames = wait_frames - 1
            return
        end
        player.gui.left.clear()
        player.gui.top.clear()
        player.gui.center.clear()

        player.teleport({0,0})
        player.zoom = 1
        
        if frame_index > 0 and frame_index <= #frames then
            game.take_screenshot({
                player = player,
                by_player = player,
                surface = player.surface,
                position = { 0, 0 },
                resolution = { 32, 32 },
                zoom = 1,
                path = frames[frame_index].path,
                show_gui = true,
                show_entity_info = false,
                anti_alias = false,
            })
            if frames[frame_index].switch then
                for _, tile in ipairs(tiles) do
                    tile.name = 'out-of-map'
                end
                player.surface.set_tiles(tiles, true)
                wait_frames = math.min(extract_interval * 10 + 30, 300)
            end
        end

        if frame_index == #frames then
            script.on_event(defines.events.on_tick, nil)
            player.gui.left.add({
                type = 'frame',
                caption = 'Graphio exporting done'
            }).add({
                type = 'label',
                caption = 'You should now quit the game, you don\'t have to save.'
            })
            log('\x01done\x04')
            return
        end

        frame_index = frame_index + 1
        if frame_index <= #frames then
            player.gui.top.add({
                type = 'sprite',
                sprite = frames[frame_index].sprite,
            })
            wait_frames = extract_interval
        end
    end)
end
