local prune_level = prune_level == nil and 1 or prune_level

script.on_init(function ()

local out = load('log(...)', '', 't')

local function filter(input, predicate)
    local output = {}
    for key, entry in pairs(input) do
        if predicate(entry) then
            output[key] = entry
        end
    end
    return output
end
local function count(input)
    local count = 0
    for _ in pairs(input) do
        count = count + 1
    end
    return count
end

local crafting_machine_prototypes = filter(game.entity_prototypes, function (entity_prototype)
    return entity_prototype.crafting_speed ~= nil
end)
local beacon_prototypes = filter(game.entity_prototypes, function (entity_prototype)
    return entity_prototype.distribution_effectivity ~= nil
end)
local item_prototypes = filter(game.item_prototypes, function (item_prototype)
    return true
end)
local fluid_prototypes = filter(game.fluid_prototypes, function (fluid_prototype)
    return true
end)
local recipe_prototypes = filter(game.recipe_prototypes, function (recipe_prototype)
    return true
end)

local function prune_prototypes()
    if prune_level == 0 then return end
    local force = game.forces.player
    local all_recipes = force.recipes

    -- Tracks which items, fluids and recipes can be obtained.
    local attainable = {
        items = {},
        fluids = {},
        recipes = {},
        updated = true
    }

    local function make_item_attainable(name)
        if not attainable.items[name] then
            attainable.updated = true
            attainable.items[name] = true
        end
    end

    local function is_item_attainable(name)
        return not not attainable.items[name]
    end

    local function make_fluid_attainable(name, temperature)
        if not temperature then
            temperature = game.fluid_prototypes[name].default_temperature
        end
        if not attainable.fluids[name] then
            attainable.updated = true
            attainable.fluids[name] = { [temperature] = true }
            return
        end
        local temperatures = attainable.fluids[name]
        if not temperatures[temperature] then
            attainable.updated = true
            temperatures[temperature] = true
        end
    end

    local function is_fluid_attainable(name, minimum_temperature, maximum_temperature)
        local temperatures = attainable.fluids[name]
        if not temperatures then return false end

        for temperature in pairs(temperatures) do
            local is_minimum_met = not minimum_temperature or (minimum_temperature <= temperature)
            local is_maximum_met = not maximum_temperature or (maximum_temperature <= temperature)
            if is_minimum_met and is_maximum_met then
                return true
            end
        end

        return false
    end

    local function make_product_attainable(product)
        if product.type == 'item' then
            make_item_attainable(product.name)
        else
            local temperature = product.temperature
            make_fluid_attainable(product.name, temperature)
        end
    end

    local function is_ingredient_attainable(ingredient)
        if ingredient.type == 'item' then
            return is_item_attainable(ingredient.name)
        else
            return is_fluid_attainable(ingredient.name, ingredient.minimum_temperature, ingredient.maximum_temperature)
        end
    end

    local function try_craft_recipe(recipe)
        if attainable.recipes[recipe.name] then return end
        -- Check if recipe is unlocked
        if not recipe.enabled then return end
        -- Check if ingredients are available
        local ingredients = recipe.ingredients
        for _, ingredient in ipairs(ingredients) do
            if not is_ingredient_attainable(ingredient) then
                return
            end
        end

        -- Recipe is craftable, so mark products as attainable
        for _, product in ipairs(recipe.products) do
            make_product_attainable(product)
        end

        attainable.recipes[recipe.name] = true
        attainable.updated = true
    end

    local function try_unlock_technology(technology)
        if technology.researched then return end
        if not technology.enabled then return end

        for _, prerequisite in pairs(technology.prerequisites) do
            if not prerequisite.researched then
                return
            end
        end
        for _, ingredient in ipairs(technology.research_unit_ingredients) do
            if not is_ingredient_attainable(ingredient) then
                return
            end
        end

        technology.researched = true
        attainable.updated = true
    end

    local function is_entity_attainable(entity_prototype)
        for item in pairs(entity_prototype.items_to_place_this) do
            if is_item_attainable(item) then
                return true
            end
        end
        return false
    end

    -- Prune level 1 and 2 both include resources that can naturally spawn/be mined
    for _, entity_prototype in pairs(game.entity_prototypes) do
        if entity_prototype.autoplace_specification then
            local products = entity_prototype.mineable_properties
            if products then products = products.products end
            if products then
                for _, product in ipairs(products) do
                    make_product_attainable(product)
                end
            end
        end
    end

    -- For 0.16, we do not have access to the output fluid box of boilers.
    -- So as a workaround, add any fluid which has a gas_temperature set to a
    -- reasonably low value (< 10K).
    for _, fluid_prototype in pairs(fluid_prototypes) do
        if fluid_prototype.gas_temperature and fluid_prototype.gas_temperature < 1e5 then
            make_fluid_attainable(fluid_prototype.name)
        end
    end

    -- Prune level 1 will mark all products from recipes that are initially unlocked
    -- as attainable, and then research all technologies.
    if prune_level == 1 then
        for _, recipe in pairs(all_recipes) do
            if recipe.enabled then
                for _, product in ipairs(recipe.products) do
                    make_product_attainable(product)
                end
            end
        end

        force.research_all_technologies()
    end

    local fluid_entity_prototypes = filter(game.entity_prototypes, function (entity_prototype)
        return not not entity_prototype.fluid
    end)
    
    while attainable.updated do
        attainable.updated = false

        -- Check if any entities that produce fluids can be placed
        for _, entity_prototype in pairs(fluid_entity_prototypes) do
            if is_entity_attainable(entity_prototype) then
                make_fluid_attainable(entity_prototype.fluid.name)
                fluid_entity_prototypes[entity_prototype.name] = nil
            end
        end

        -- Check if any recipe can be crafted
        for _, recipe in pairs(all_recipes) do
            try_craft_recipe(recipe)
        end

        -- For prune levels over 1, no technologies were researched yet.
        -- It therefore has to be done manually.
        if prune_level > 1 then
            for _, technology in pairs(force.technologies) do
                try_unlock_technology(technology)
            end
        end
    end

    -- Take the pruned information and filter prototypes based on them
    crafting_machine_prototypes = filter(crafting_machine_prototypes, function (crafting_machine_prototype)
        if is_entity_attainable(crafting_machine_prototype) then
            return true
        end
        out(string.format('pruned crafting machine %q', crafting_machine_prototype.name))
        return false
    end)
    beacon_prototypes = filter(beacon_prototypes, function (beacon_prototype)
        if is_entity_attainable(beacon_prototype) then
            return true
        end
        out(string.format('pruned beacon %q', beacon_prototype.name))
        return false
    end)
    recipe_prototypes = filter(recipe_prototypes, function (recipe_prototype)
        if attainable.recipes[recipe_prototype.name] then
            return true
        end
        out(string.format('pruned recipe %q', recipe_prototype.name))
        return false
    end)
    item_prototypes = filter(item_prototypes, function (item_prototype)
        if attainable.items[item_prototype.name] then
            return true
        end
        out(string.format('pruned item %q', item_prototype.name))
        return false
    end)
    fluid_prototypes = filter(fluid_prototypes, function (fluid_prototype)
        if attainable.fluids[fluid_prototype.name] then
            return true
        end
        out(string.format('pruned fluid %q', fluid_prototype.name))
        return false
    end)
end

prune_prototypes()

local write_template_str = { '', '\x02', nil, '\x03' }
local function write_str(entry)
    if type(entry) == 'number' then
        entry = tostring(entry)
    end
    if type(entry) ~= 'string' then
        error('expected string, got something else', 2)
    end
    write_template_str[3] = entry
    out(write_template_str)
end
local write_template_loc = { '', '\x02', nil, '\x1f', nil, '\x03' }
local function write_loc(entry)
    if type(entry) ~= 'table' then
        error('expected table, got something else', 2)
    end
    if type(entry[1]) ~= 'string' then
        error('expected first entry to be a string', 2)
    end
    write_template_loc[3] = entry[1]
    write_template_loc[5] = entry
    out(write_template_loc)
end
local function write_allowed_effects(allowed_effects)
    if not allowed_effects then
        write_str('0000')
        return
    end
    local energy = allowed_effects.consumption and '1' or '0'
    local speed = allowed_effects.speed and '1' or '0'
    local productivity = allowed_effects.productivity and '1' or '0'
    local pollution = allowed_effects.pollution and '1' or '0'
    write_str(energy .. speed .. productivity .. pollution)
end

out({ '',
    '\x01\x02',
    tostring(count(crafting_machine_prototypes)), '\x1f',
    tostring(count(beacon_prototypes)), '\x1f',
    tostring(count(recipe_prototypes)), '\x1f',
    tostring(count(item_prototypes)), '\x1f',
    tostring(count(fluid_prototypes)), '\x03'})

local crafting_machine_categories = {}
local function add_crafting_machine_category(crafting_machine_name, category)
    local machines = crafting_machine_categories[category]
    if not machines then
        machines = {}
        crafting_machine_categories[category] = machines
    end
    for _, machine in ipairs(machines) do
        if machine == crafting_machine_name then
            return
        end
    end
    table.insert(machines, crafting_machine_name)
    table.sort(machines)
end

for _, crafting_machine_prototype in pairs(crafting_machine_prototypes) do
    write_str(crafting_machine_prototype.name)
    write_loc(crafting_machine_prototype.localised_name)
    write_loc(crafting_machine_prototype.localised_description)
    write_str(crafting_machine_prototype.crafting_speed)

    local energy_consumption
    local energy_drain
    local electric_energy_source_prototype = crafting_machine_prototype.electric_energy_source_prototype
    local burner_prototype = crafting_machine_prototype.burner_prototype
    if electric_energy_source_prototype then
        energy_consumption = crafting_machine_prototype.max_energy_usage * 60
        energy_drain = electric_energy_source_prototype.drain * 60
    elseif burner_prototype then
        energy_consumption = crafting_machine_prototype.max_energy_usage / burner_prototype.effectivity * 60
        energy_drain = 0
    else
        error('unknown energy source for machine')
    end
    write_str(energy_consumption)
    write_str(energy_drain)

    local module_slots = crafting_machine_prototype.module_inventory_size or 0
    write_str(module_slots)

    write_allowed_effects(crafting_machine_prototype.allowed_effects)

    for category in pairs(crafting_machine_prototype.crafting_categories) do
        add_crafting_machine_category(crafting_machine_prototype.name, category)
    end
end

for _, beacon_prototype in pairs(beacon_prototypes) do
    write_str(beacon_prototype.name)
    write_loc(beacon_prototype.localised_name)
    write_loc(beacon_prototype.localised_description)
    write_str(beacon_prototype.distribution_effectivity)
    write_allowed_effects(beacon_prototype.allowed_effects)
end

for _, recipe_prototype in pairs(recipe_prototypes) do
    write_str(recipe_prototype.name)
    write_loc(recipe_prototype.localised_name)
    write_loc(recipe_prototype.localised_description)
    write_str(recipe_prototype.energy)

    local ingredients = recipe_prototype.ingredients
    local products = recipe_prototype.products

    write_str(#ingredients)
    for _, ingredient in ipairs(ingredients) do
        write_str(ingredient.type)
        write_str(ingredient.name)
        write_str(ingredient.amount)
        local catalyst_amount = ingredient.catalyst_amount
        if not catalyst_amount then
            catalyst_amount = 0
            for _, product in ipairs(products) do
                if product.amount and
                    product.type == ingredient.type and
                    product.name == ingredient.name then
                    catalyst_amount = math.min(ingredient.amount, product.amount)
                    break
                end
            end
        end
        write_str(catalyst_amount)
        if ingredient.type == 'fluid' then
            local flags = (ingredient.minimum_temperature and '1' or '0')
                .. (ingredient.maximum_temperature and '1' or '0')
            write_str(flags)
            if ingredient.minimum_temperature then
                write_str(ingredient.minimum_temperature)
            end
            if ingredient.maximum_temperature then
                write_str(ingredient.maximum_temperature)
            end
        end
    end

    write_str(#products)
    for _, product in ipairs(products) do
        write_str(product.type)
        write_str(product.name)
        if product.type == 'fluid' then
            if product.temperature then
                write_str(product.temperature)
            else
                write_str(fluid_prototypes[product.name].default_temperature)
            end
        end
        if product.amount then
            write_str('fixed')
            write_str(product.amount)
            local catalyst_amount = product.catalyst_amount
            if not catalyst_amount then
                catalyst_amount = 0
                for _, ingredient in ipairs(ingredients) do
                    if product.type == ingredient.type and
                        product.name == ingredient.name then
                        catalyst_amount = math.min(ingredient.amount, product.amount)
                        break
                    end
                end
            end
            write_str(catalyst_amount)
        else
            write_str('probability')
            write_str(product.amount_min)
            write_str(product.amount_max)
            write_str(product.probability)
        end
    end

    local machines = crafting_machine_categories[recipe_prototype.category] or {}
    local filtered_machines = {}
    for _, machine in ipairs(machines) do
        local ingredient_count = crafting_machine_prototypes[machine].ingredient_count
        if not ingredient_count or ingredient_count >= #ingredients then
            table.insert(filtered_machines, machine)
        end 
    end
    write_str(#filtered_machines)
    for _, machine in ipairs(filtered_machines) do
        write_str(machine)
    end
end

for _, item_prototype in pairs(item_prototypes) do
    write_str(item_prototype.name)
    write_loc(item_prototype.localised_name)
    write_loc(item_prototype.localised_description)
    
    local module_effects = item_prototype.module_effects
    write_str(module_effects and '1' or '0')
    if module_effects then
        write_str(module_effects.consumption and module_effects.consumption.bonus or 0)
        write_str(module_effects.speed and module_effects.speed.bonus or 0)
        write_str(module_effects.productivity and module_effects.productivity.bonus or 0)
        write_str(module_effects.pollution and module_effects.pollution.bonus or 0)

        local limitations = item_prototype.limitations
        local has_limitations = (type(limitations) == 'table' and #limitations > 0)
        write_str(has_limitations and '1' or '0')
        if has_limitations then
            local filtered_limitations = {}
            for _, limitation in ipairs(limitations) do
                if recipe_prototypes[limitation] then
                    table.insert(filtered_limitations, limitation)
                end
            end
            write_str(#filtered_limitations)
            for _, limitation in ipairs(filtered_limitations) do
                write_str(limitation)
            end
        end
    end
end

for _, fluid_prototype in pairs(fluid_prototypes) do
    write_str(fluid_prototype.name)
    write_loc(fluid_prototype.localised_name)
    write_loc(fluid_prototype.localised_description)
end

out('\x04')

error('done')

end)
